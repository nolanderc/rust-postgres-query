//! Helper macros and traits built around
//! [tokio-postgres](https://docs.rs/tokio-postgres/0.5.1/tokio_postgres/index.html) to define
//! queries with human readable parameters and return values.
//!
//! # Example
//!
//! ```
//! # use tokio_postgres::Client;
//! # use postgres_query::{query, FromSqlRow, Result};
//! # fn connect() -> Client { unimplemented!() }
//! # async fn foo() -> Result<()> {
//! // Connect to the database
//! let client: Client = connect(/* ... */);
//!
//! // Construct the query
//! let query = query!(
//!     "SELECT age, name FROM people WHERE age >= $min_age",
//!     min_age = 18
//! );
//!
//! // Define the structure of the data returned from the query
//! #[derive(FromSqlRow)]
//! struct Person {
//!     age: i32,
//!     name: String,
//! }
//!
//! // Execute the query
//! let people: Vec<Person> = query.fetch(&client).await?;
//!
//! for person in people {
//!     println!("{} is {} years young", person.name, person.age);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Queries
//!
//! The preferred way of constructing a new [`Query`] is through the [`query!`] macro. It uses a
//! syntax similar to the `format!(...)` family of macros from the standard library. The first
//! parameter is the SQL query and is always given as a string literal (this might be relaxed in the
//! future).  This string literal may contain parameter bindings on the form `$ident` where `ident`
//! is any valid Rust identifier (`$abc`, `$value_123`, etc.).
//!
//! ```
//! # use postgres_query::query;
//! let age = 42;
//! let insert_person = query!(
//!     "INSERT INTO people VALUES ($age, $name)",
//!     name = "John Wick", // Binds "$name" to "John Wick"
//!     age,                // Binds "$age" to the value of `age`
//! );
//! ```
//!
//! During compilation the query is converted into the format expected by PostgreSQL: parameter
//! bindings are converted to using numbers ($1, $2, etc.) and the actual parameter values are put
//! into a 1-indexed array. The code snippet above would be expanded into the following:
//!
//! ```
//! # use postgres_query::*;
//! let age = 42;
//! let insert_person = Query::new_static(
//!     "INSERT INTO people VALUES ($1, $2)",
//!     vec![&age, &"John Wick"],
//! );
//! ```
//!
//! # Data Extraction
//!
//! In addition to helping you define new queries this crate provides the [`FromSqlRow`] trait which
//! makes it easy to extract typed values from the resulting rows. The easiest way to implement this
//! trait for new `struct`s is to use the included [`derive(FromSqlRow)`] macro.
//!
//! - If used on a tuple struct, values will be extracted from the corresponding columns based on
//! their position in the tuple.
//! - If used on a stuct with named fields, values will be extracted from the column with the same
//! name as the field.
//!
//! ```
//! # use postgres_query::*;
//! #[derive(FromSqlRow)]
//! struct TupleData(i32, String);
//!
//! #[derive(FromSqlRow)]
//! struct NamedData {
//!     age: i32,
//!     name: String,
//! };
//! ```
//!
//! # Caching queries
//!
//! From time to time you probably want to execute the same query multiple times, but with different
//! parameters. In times like these we can decrease the load on the database by preparing our
//! queries before executing them. By wrapping a client in a [`Caching`] struct this behaviour is
//! automatically provided for all queries that originate from this crate:
//!
//! ```rust
//! # use tokio_postgres::Client;
//! # use postgres_query::{query, Result, Caching};
//! # fn connect() -> Client { unimplemented!() }
//! # async fn foo() -> Result<()> {
//! // Connect to the database
//! let client: Client = connect(/* ... */);
//!
//! // Wrap the client in a query cache
//! let cached_client = Caching::new(client);
//!
//! for age in 0..100i32 {
//!     let query = query!("SELECT name, weight FROM people WHERE age = $age", age);
//!
//!     // The query is prepared and cached the first time it's executed.
//!     // All subsequent fetches will use the cached Statement.
//!     let people: Vec<(String, i32)> = query.fetch(&cached_client).await?;
//!     
//!     /* Do something with people */
//! }
//! # Ok(())
//! # }
//! ```
//!
//! [`Query`]: struct.Query.html
//! [`query!`]: macro.Query.html
//! [`FromSqlRow`]: extract/trait.FromSqlRow.html
//! [`derive(FromSqlRow)`]: derive.FromSqlRow.html
//! [`Caching`]: client/struct.Caching.html

pub mod client;
pub mod execute;
pub mod extract;

mod error;
mod parse;

use postgres_types::ToSql;
use proc_macro_hack::proc_macro_hack;
use std::ops::Deref;

pub use crate::client::Caching;
pub use crate::error::{Error, Result};
pub use crate::extract::FromSqlRow;

/// Extract values from a row.
///
/// - If used on a tuple struct, values will be extracted from the corresponding columns based on
/// their position in the tuple.
/// - If used on a stuct with named fields, values will be extracted from the column with the same
/// name as the field.
///
/// # Example
///
/// ```
/// # use postgres_query::*;
/// #[derive(FromSqlRow)]
/// struct TupleData(i32, String);
///
/// #[derive(FromSqlRow)]
/// struct NamedData {
///     age: i32,
///     name: String,
/// };
/// ```
pub use postgres_query_macro::FromSqlRow;

/// Constructs a new query at compile-time. See also `query_dyn!`.
///
/// # Usage
///
/// This macro expands to an expression with the type `Query`.
///
/// The first parameter is the SQL query and is always given as a string literal. This string
/// literal may contain parameter bindings on the form `$ident` where `ident` is any valid Rust
/// identifier (`$abc`, `$value_123`, etc.). The order of the parameters does not matter.
///
/// ```
/// # use postgres_query::query;
/// let age = 42;
/// let insert_person = query!(
///     "INSERT INTO people VALUES ($age, $name)",
///     name = "John Wick", // Binds "$name" to "John Wick"
///     age,                // Binds "$age" to the value of `age`
/// );
/// ```
///
/// During compilation the query is converted into the format expected by PostgreSQL: parameter
/// bindings are converted to using numbers (`$1`, `$2`, etc.) and the actual parameter values are
/// put into a 1-indexed array. The code snippet above would be expanded into the following:
///
/// ```
/// # use postgres_query::*;
/// let age = 42;
/// let insert_person = Query::new_static(
///     "INSERT INTO people VALUES ($1, $2)",
///     vec![&age, &"John Wick"],
/// );
/// ```
#[macro_export]
macro_rules! query {
    ($($tt:tt)*) => {
        $crate::__query_static!($($tt)*)
    };
}

/// Constructs a new query dynamically at runtime. See also `query!`.
///
/// # Usage
///
/// This macro expands to an expression with the type `Result<Query>`.
///
/// The first parameter is the SQL query and is always given as a `&str`. This string may contain
/// parameter bindings on the form `$ident` where `ident` is any valid Rust identifier (`$abc`,
/// `$value_123`, etc.). The order of the parameters does not matter.
///
/// ```
/// # use postgres_query::{query_dyn, Result};
/// # fn foo() -> Result<()> {
/// // We can construct the actual query at runtime
/// let mut sql = "INSERT INTO people VALUES".to_owned();
/// sql.push_str("($age, $name)");
///
/// let age = 42;
///
/// let insert_person = query_dyn!(
///     &sql,
///     name = "John Wick", // Binds "$name" to "John Wick"
///     age,                // Binds "$age" to the value of `age`
/// )?;
/// # Ok(())
/// # }
/// ```
///
/// The query and all the parameters are passed into `Query::parse`, so the above would be expanded
/// into:
///
/// ```
/// # use postgres_query::Query;
/// // We can construct the actual query at runtime
/// let mut sql = "INSERT INTO people VALUES".to_string();
/// sql.push_str("($age, $name)");
///
/// let age = 42;
///
/// let insert_person = Query::parse(
///     &sql,
///     &[("name", &"John Wick"), ("age", &age)],
/// );
/// ```
///
///
/// ## Dynamic Binding
///
/// Optionally, you may also choose to include additional bindings at runtime by using the
/// `..bindings` syntax. This is supported for any type that implements `IntoIterator<Item = (&str,
/// Parameter)>`, ie. `Vec<(&str, Parameter)>`, `HashMap<&str, Parameter>`, `Option<(&str,
/// Parameter)>`, iterators, and so on.
///
/// Dynamic bindings may be mixed with static bindings:
///
/// ```
/// # use postgres_query::{query_dyn, Parameter, Result};
/// # fn foo() -> Result<()> {
/// let mut bindings = Vec::new();
///
/// // We use the `as Parameter` to please the type checker.
/// // Alternatively, we could specify the type for bindings: `Vec<(&str, Parameter)>`.
/// bindings.push(("age", &42 as Parameter));
/// bindings.push(("name", &"John Wick" as Parameter));
///
/// let sql = "INSERT INTO people VALUES ($age, $name, $height)".to_string();
/// let insert_person = query_dyn!(
///     &sql,
///     height = 192,
///     ..bindings,
/// )?;
/// # Ok(())
/// # }
/// ```
///
///
/// # A larger example
///
/// Let's say that we wanted to dynamically add filters to our query:
///
/// ```
/// # use postgres_query::{query_dyn, Parameter, Query, Result};
/// # fn foo() -> Result<()> {
/// // We have the query we want to execute
/// let mut sql = "SELECT * FROM people".to_string();
///
/// // and some filters we got from the user.
/// let age_filter: Option<i32> = Some(32);
/// let name_filter: Option<&str> = None;
///
/// // Then we dynamically build a list of filters and bindings to use:
/// let mut filters = Vec::new();
/// let mut bindings = Vec::new();
///
/// // We add the filters as needed.
/// if let Some(age) = age_filter.as_ref() {
///     filters.push("age > $min_age");
///     bindings.push(("min_age", age as Parameter));
/// }
///
/// if let Some(name) = name_filter.as_ref() {
///     filters.push("name LIKE $name");
///     bindings.push(("name", name as Parameter));
/// }
///
/// // And add them to the query.
/// if filters.len() > 0 {
///     sql += &format!(" WHERE {}", filters.join(" AND "));
/// }
///
/// // Then we can use it as normal.
/// let query: Query = query_dyn!(&sql, ..bindings)?;
/// # Ok(())
/// # }
/// ```
#[macro_export]
macro_rules! query_dyn {
    ($($tt:tt)*) => {
        $crate::__query_dynamic!($($tt)*)
    };
}

#[proc_macro_hack]
#[doc(hidden)]
pub use postgres_query_macro::{query_dynamic as __query_dynamic, query_static as __query_static};

/// A shorthand for types that can be treated as SQL parameters.
///
/// A common use case for this type alias is when using dynamic bindings and you have to please the
/// type checker:
///
/// ```
/// # use postgres_query::{Parameter, query_dyn, Result};
/// # fn foo() -> Result<()> {
/// let mut bindings = Vec::new();
///
/// // Without the `as Parameter` the compiler assumes the type to be `&i32`.
/// bindings.push(("age", &32 as Parameter));
///
/// // Which would cause problems when adding something that is not an integer.
/// bindings.push(("name", &"John" as Parameter));
///
/// let query = query_dyn!(
///     "SELECT * FROM people WHERE age > $age AND name = $name",
///     ..bindings
/// )?;
/// # Ok(())
/// # }
/// ```
///
/// Alternatively we could just set the type on the container explicitly:
///
/// ```
/// # use postgres_query::Parameter;
/// let mut bindings: Vec<(&str, Parameter)> = Vec::new();
/// ```
pub type Parameter<'a> = &'a (dyn ToSql + Sync);

/// A static query with dynamic parameters.
///
/// # Usage
///
/// ## Constructing
///
/// The preferred way of constructing a [`Query`] is by using the [`query!`] and [`query_dyn!`]
/// macros.
///
/// You may also use the `Query::parse`, `Query::new_static` or `Query::new` methods.
///
///
/// ## Executing
///
/// When executing the query you have two options, either:
///
/// 1. use the provided methods: `execute`, `fetch`, `query`, etc.
/// 2. use the `sql` and `parameters` fields as arguments to the standard [`Client`] methods
///
/// ```
/// # use tokio_postgres::{Client, Row};
/// # use postgres_query::{query, FromSqlRow, Result};
/// # fn connect() -> Client { unimplemented!() }
/// # async fn foo() -> Result<(), Box<dyn std::error::Error>> {
/// #[derive(FromSqlRow)]
/// struct Person {
///     age: i32,
///     name: String,
/// }
///
/// let client: Client = connect(/* ... */);
/// let query = query!("SELECT age, name FROM people");
///
/// // Option 1
/// let people: Vec<Person> = query.fetch(&client).await?;
///
/// // Option 2
/// let rows: Vec<Row> = client.query(query.sql(), query.parameters()).await?;
/// let people: Vec<Person> = Person::from_row_multi(&rows)?;
/// # Ok(())
/// # }
/// ```
///
/// [`Query`]: struct.Query.html
/// [`query!`]: macro.query.html
/// [`query_dyn!`]: macro.query_dyn.html
/// [`Client`]: https://docs.rs/tokio-postgres/0.5.1/tokio_postgres/struct.Client.html
#[derive(Debug, Clone)]
pub struct Query<'a> {
    sql: Sql,
    parameters: Vec<Parameter<'a>>,
}

#[derive(Debug, Clone)]
enum Sql {
    Static(&'static str),
    Dynamic(String),
}

impl<'a> Query<'a> {
    /// Create a new query an already prepared string.
    ///
    /// IMPORTANT: This does not allow you to pass named parameter bindings (`$name`, `$abc_123`,
    /// etc.). For that behaviour, refer to the `query!` macro. Instead bindings and parameters are
    /// given in the same format required by `tokio_postgres` (`$1`, `$2`, ...).
    pub fn new(sql: String, parameters: Vec<Parameter<'a>>) -> Query<'a> {
        Query {
            sql: Sql::Dynamic(sql),
            parameters,
        }
    }

    /// Create a new query with a static query string.
    ///
    /// IMPORTANT: This does not allow you to pass named parameter bindings (`$name`, `$abc_123`,
    /// etc.), For that behaviour, refer to the `query_dyn!` macro. Instead bindings and parameters
    /// are given in the same format required by `tokio_postgres` (`$1`, `$2`, ...).
    pub fn new_static(sql: &'static str, parameters: Vec<Parameter<'a>>) -> Query<'a> {
        Query {
            sql: Sql::Static(sql),
            parameters,
        }
    }

    /// Parses a string that may contain parameter bindings on the form `$abc_123`. This is the same
    /// function that is called when passing dynamically generated strings to the `query_dyn!`
    /// macro.
    ///
    /// Because this is a function there will some runtime overhead unlike the `query!` macro which
    /// has zero overhead when working with string literals.
    pub fn parse(text: &str, bindings: &[(&str, Parameter<'a>)]) -> Result<Query<'a>> {
        let (sql, parameters) = parse::parse(text, bindings)?;

        Ok(Query {
            sql: Sql::Dynamic(sql),
            parameters,
        })
    }

    /// Get this query as an SQL string.
    pub fn sql(&'a self) -> &'a str {
        &self.sql
    }

    /// Get the parameters of this query in the order expected by the query returned by
    /// `Query::sql`.
    pub fn parameters(&'a self) -> &[Parameter<'a>] {
        &self.parameters
    }
}

impl Deref for Sql {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        match self {
            Sql::Static(text) => text,
            Sql::Dynamic(text) => &text,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ParseError;

    macro_rules! is_match {
        ($expr:expr, $pattern:pat) => {
            match $expr {
                $pattern => true,
                _ => false,
            }
        };
    }

    #[test]
    fn parse_query_without_bindings() {
        let query = Query::parse("SELECT 123, 'abc'", &[]).unwrap();
        assert_eq!(query.sql(), "SELECT 123, 'abc'");
    }

    #[test]
    fn parse_query_single_binding() {
        let query = Query::parse("SELECT $number", &[("number", &123)]).unwrap();
        assert_eq!(query.sql(), "SELECT $1");
    }

    #[test]
    fn parse_query_missing_identifier_eof() {
        let query = Query::parse("SELECT $", &[]);
        assert!(is_match!(
            query.unwrap_err(),
            Error::Parse(ParseError::EmptyIdentifier { found: None })
        ));
    }

    #[test]
    fn parse_query_missing_identifier() {
        let query = Query::parse("SELECT $ FROM users", &[]);
        assert!(is_match!(
            query.unwrap_err(),
            Error::Parse(ParseError::EmptyIdentifier { found: Some(' ') })
        ));
    }
}
