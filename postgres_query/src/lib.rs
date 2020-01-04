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
//! let insert_person = Query {
//!     sql: "INSERT INTO people VALUES ($1, $2)",
//!     parameters: vec![&age, &"John Wick"],
//! };
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

use postgres_types::ToSql;
use proc_macro_hack::proc_macro_hack;

pub use client::Caching;
pub use error::{Error, Result};
pub use extract::FromSqlRow;

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
///
/// # Attributes
///
/// Data extraction can be customized by using the `row` attribute.
///
///
/// ## Container attributes
///
/// These attributes are put on the struct itself.
///
///
/// ### `#[row(order)]`
/// 
/// On structs with named fields: uses the order of the fields instead of their names. This
/// overrides the default behaviour. 
///
///
/// ## Field attributes
///
/// These attributes are put on the fields of the struct.
///
///
/// ### `#[row(rename = "...")]`
/// 
/// Use a name other than that of the field when looking up the name of the column.
/// 
///
/// ### `#[row(flatten)]`
/// 
/// Flatten the contents of this field into its container by recursively calling `FromSqlRow` on the
/// field's type. This removes one level of nesting:
///
/// ```
/// # use postgres_query::{FromSqlRow, query, Result};
/// # use tokio_postgres::Client;
/// # async fn foo() -> Result<()> {
/// # let client: Client = unimplemented!();
/// #[derive(FromSqlRow)]
/// struct Person {
///     name: String,
///     age: i32
/// }
///
/// #[derive(FromSqlRow)]
/// struct Customer {
///     id: i32,
///     #[row(flatten)]
///     info: Person,
/// }
///
/// let customer: Customer = query!("SELECT 14 as id, 'Bob' as name, 47 as age")
///     .fetch_one(&client)
///     .await?;
///
/// assert_eq!(customer.id, 14);
/// assert_eq!(customer.info.name, "Bob");
/// assert_eq!(customer.info.age, 47);
/// # Ok(())
/// # }
/// ```
///
pub use postgres_query_macro::FromSqlRow;

/// Constructs a new query.
///
/// # Usage
///
/// The first parameter is the SQL query and is always given as a string literal (this might be
/// relaxed in the future).  This string literal may contain parameter bindings on the form `$ident`
/// where `ident` is any valid Rust identifier (`$abc`, `$value_123`, etc.). The order of the
/// parameters does not matter.
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
/// let insert_person = Query {
///     sql: "INSERT INTO people VALUES ($1, $2)",
///     parameters: vec![&age, &"John Wick"],
/// };
/// ```
#[macro_export]
macro_rules! query {
    ($sql:literal) => {
        $crate::__query!($sql)
    };
    ($sql:literal, $($arg:expr),* $(,)?) => {
        $crate::__query!($sql, $($arg),*)
    };
}

#[proc_macro_hack]
#[doc(hidden)]
pub use postgres_query_macro::query as __query;

/// A static query with dynamic parameters.
///
/// # Usage
///
/// The preferred way of constructing a [`Query`] is by using the [`query!`] macro.
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
/// let rows: Vec<Row> = client.query(query.sql, &query.parameters).await?;
/// let people: Vec<Person> = Person::from_row_multi(&rows)?;
/// # Ok(())
/// # }
/// ```
///
/// [`Query`]: struct.Query.html
/// [`query!`]: macro.query.html
/// [`Client`]: https://docs.rs/tokio-postgres/0.5.1/tokio_postgres/struct.Client.html
#[derive(Debug, Clone)]
pub struct Query<'a> {
    pub sql: &'static str,
    pub parameters: Vec<&'a (dyn ToSql + Sync)>,
}
