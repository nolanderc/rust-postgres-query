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
//!
//! ## Dynamic Queries
//!
//! If necessary, queries may be constructed from `&str`s at runtime instead of the usual
//! compile-time string literals expected by the `query!` macro. This is achieved by using the
//! [`query_dyn!`] macro instead. In addition to dynamic queries, parameter bindings may also be
//! dynamically: 
//!
//! ```
//! # use postgres_query::*;
//! let mut sql = "SELECT * FROM people WHERE name = $name".to_string();
//! let mut bindings = Vec::new();
//!
//! // Add a filter at runtime
//! sql += " AND age > $min_age";
//! bindings.push(("min_age", &42 as Parameter));
//!
//! let query: Result<Query> = query_dyn!(
//!     &sql,
//!     name = "John",
//!     ..bindings,
//! );
//! ```
//!
//! Using dynamic queries does introduce some errors that cannot be caught at runtime: such as some
//! parameters in the query not having a matching binding. Because of this the value returned by the
//! [`query_dyn!`] macro is not a `Query` but a `Result<Query>` which carries an error you must
//! handle:
//!
//! ```
//! # use postgres_query::*;
//! let mut sql = "SELECT * FROM people".to_string();
//! sql += " WHERE age <= $max_age AND name = $name";
//!
//! let query: Result<Query> = query_dyn!(
//!     &sql,
//!     name = "John",
//!     // Forgot to bind the parameter `max_age`. 
//!     // Will result in an error.
//! );
//!
//! assert!(query.is_err());
//! ```
//! 
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
//! ## Multi-mapping
//!
//! If you query the same table multiple times it gets tedious to have to redefine structs with the
//! same fields over and over. Preferably we would like to reuse the same definition multiple times.
//! We can do this be utilizing "multi-mapping".
//!
//!
//! ### Partitions
//!
//! Multi-mapping works by splitting the columns of rows returned by a query into multiple
//! partitions (or slices). For example, if we had the query `SELECT books.*, authors.* FROM ...`,
//! we would like to extract the data into two structs: `Book` and `Author`. We accomplish this by
//! looking at the columns returned by the database and splitting them into partitions:
//!
//! ```text
//! Columns:    id, title, release_date, genre, id, name, birthyear
//! Partitions: +------------Book-------------+ +------Author-----+
//! ```
//!
//!
//! ### Partitioning schemes
//!
//! There are two supported ways to partition a row: either we specify the number of columns
//! required to populate each struct (in the example above: 4 columns for Book and 3 for author), or
//! we split on the name of a column. The former should generally only be used when you know the
//! number of columns isn't going to change. The latter is less prone to break provided you choose
//! an appropriate column to split on (a good candidate is usually `id` as almost all tables have
//! this as their first
//! column).
//!
//! You choose which partitioning scheme you want to use by using the provided
//! [attributes](./derive.FromSqlRow.html#attributes). In order to accomplish the partitioning in
//! the example above we could split on the column name `id`:
//!
//! ```
//! # use postgres_query::FromSqlRow;
//! #[derive(FromSqlRow)]
//! struct Book {
//!     id: i32,
//!     title: String,
//!     release_date: String,
//!     genre: String,
//! }
//!
//! #[derive(FromSqlRow)]
//! struct Author {
//!     id: i32,
//!     name: String,
//!     birthyear: i32,
//! }
//!
//! #[derive(FromSqlRow)]
//! #[row(split)]
//! struct BookAuthor {
//!     #[row(flatten, split = "id")]
//!     book: Book,
//!     #[row(flatten, split = "id")]
//!     author: Author,
//! }
//! ```
//!
//! Alternatively, we can make `Author` a part of the `Book` struct:
//!
//! ```
//! # use postgres_query::FromSqlRow;
//! #[derive(FromSqlRow)]
//! struct Author {
//!     id: i32,
//!     name: String,
//!     birthyear: i32,
//! }
//!
//! #[derive(FromSqlRow)]
//! #[row(split)]
//! struct Book {
//!     #[row(split = "id")]
//!     id: i32,
//!     title: String,
//!     release_date: String,
//!     genre: String,
//!
//!     #[row(flatten, split = "id")]
//!     author: Author,
//! }
//! ```
//!
//! ### Many-to-one Relationships
//! 
//! In the previous examples we had a `Book` that contained an `Author`. This is what is called a
//! many-to-one relationship, since one book only has one author, but many books may share the same
//! author (or so we assume anyway). What if you instead had `Author` an author that contained many
//! `Book`s? We know that one author may write many books, so that is a one-to-many relationship. We
//! can write an extractor for that case as well:
//! 
//! ```
//! # use postgres_query::*;
//! # use tokio_postgres::Client;
//! # async fn foo() -> Result<()> {
//! # let client: Client = unimplemented!();
//! #[derive(FromSqlRow)]
//! #[row(split, group)]
//! struct Author {
//!     #[row(split = "id", key)]
//!     id: i32,
//!     name: String,
//!     birthyear: i32,
//!
//!     #[row(split = "id", merge)]
//!     books: Vec<Book>,
//! }
//!
//! #[derive(FromSqlRow)]
//! struct Book {
//!     id: i32,
//!     title: String,
//!     release_date: String,
//!     genre: String,
//! }
//!
//! let authors: Vec<Author> = query!(
//!         "SELECT authors.*, books.*
//!          INNER JOIN books ON books.author = authors.id
//!          GROUP BY authors.id"
//!     )
//!     .fetch(&client)
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! See the section on [attributes](./derive.FromSqlRow.html#attributes) for a more advanced
//! in-depth explanation of multi-mapping.
//!
//!
//! # Caching queries
//!
//! From time to time you probably want to execute the same query multiple times, but with different
//! parameters. In times like these we can decrease the load on the database by preparing our
//! queries before executing them. By wrapping a client in a [`Caching`] struct this behaviour is
//! automatically provided for all queries that originate from this crate:
//!
//! ```
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
//! [`query!`]: macro.query.html
//! [`query_dyn!`]: macro.query_dyn.html
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
///
///
/// # Attributes
///
/// Data extraction can be customized by using the `#[row(...)]` attribute. Attributes can be
/// separated into two categories, those which go on the container itself:
///
/// - [`#[row(exact)]`](#rowexact)
/// - [`#[row(split)]`](#rowsplit)
/// - [`#[row(group)]`](#rowgroup)
/// - [`#[row(hash)]`](#rowhash)
///
/// and those which are placed on the container's fields:
///
/// - [`#[row(rename = "...")]`](#rowrename--)
/// - [`#[row(flatten)]`](#rowflatten)
/// - [`#[row(stride = N)]`](#rowstride--n)
/// - [`#[row(split = "...")]`](#rowsplit--)
/// - [`#[row(key)]`](#rowkey)
/// - [`#[row(merge)]`](#rowmerge)
///
///
/// ## Container attributes
///
/// These attributes are put on the struct itself.
///
///
/// ### `#[row(exact)]`
///
/// [Partition](./index.html#multi-mapping) the row according to the number of columns matched by
/// each group.
///
/// Note that no order is forced upon fields within any group. In the example below, that means that
/// even though the `generation` and `origin` fields are flipped relative to the query, the
/// extraction will be successful:
///
/// ```
/// # use postgres_query::{FromSqlRow, Result, query};
/// # use tokio_postgres::Client;
/// # async fn foo() -> Result<()> {
/// # let client: Client = unimplemented!();
/// #[derive(FromSqlRow)]
/// #[row(exact)]
/// struct Family {
///     generation: i32,
///     origin: String,
///     #[row(flatten)]
///     parent: Person,
///     #[row(flatten)]
///     child: Person,
/// }
///
/// #[derive(FromSqlRow)]
/// struct Person {
///     id: i32,
///     name: String,
/// }
///
/// let family = query!(
///     "SELECT
///         'Germany' as origin, 7 as generation,
///         1 as id, 'Bob' as name,
///         2 as id, 'Ike' as name"
///     )
///     .fetch_one::<Family, _>(&client)
///     .await?;
/// # Ok(())
/// # }
/// ```
///
/// ### `#[row(split)]`
///
/// [Partition](./index.html#multi-mapping) the row according to the field's [split
/// points](extract/fn.split_columns_many.html#split-points).
///
/// Split points are introduced by using the [`#[row(split = "...")]`](#rowsplit---1) attribute on
/// fields.
///
/// ```
/// # use postgres_query::{FromSqlRow, Result, query};
/// # use tokio_postgres::Client;
/// # async fn foo() -> Result<()> {
/// # let client: Client = unimplemented!();
/// #[derive(FromSqlRow)]
/// #[row(split)]
/// struct Family {
///     generation: i32,
///     origin: String,
///     #[row(flatten, split = "id")]
///     parent: Person,
///     #[row(flatten, split = "id")]
///     child: Person,
/// }
///
/// #[derive(FromSqlRow)]
/// struct Person {
///     id: i32,
///     name: String,
/// }
///
/// let family = query!(
///     "SELECT
///         'Germany' as origin, 7 as generation,
///         1 as id, 'Bob' as name,
///         2 as id, 'Ike' as name"
///     )
///     .fetch_one::<Family, _>(&client)
///     .await?;
/// # Ok(())
/// # }
/// ```
///
///
/// ### `#[row(group)]`
///
/// Enables one-to-many mapping for the container. One-to-many mapping requires that at least one
/// field has the `#[row(key)]` attribute and that one other field has the `#[row(merge)]` attribute.
///
/// When extracting values from multiple rows, any two **adjacent** rows that are identical on their
/// fields marked with `#[row(key)]` will have their fields tagged with `#[row(merge)]` merged. This
/// means that in order to get the expected relation back, you may need to include a `GROUP BY`
/// statement in your SQL query, hence the name `group`.
///
/// ```
/// # use postgres_query::*;
/// # use tokio_postgres::Client;
/// # async fn foo() -> Result<()> {
/// # let client: Client = unimplemented!();
/// #[derive(Debug, FromSqlRow)]
/// #[row(group)]
/// struct Author {
///     #[row(key)]
///     name: String,
///
///     #[row(merge)]
///     books: Vec<Book>,
/// }
///
/// #[derive(Debug, FromSqlRow)]
/// struct Book {
///     title: String,
/// }
///
/// let authors = query!(
///         "SELECT 'J.R.R. Tolkien' as name, 'The Fellowship of the Ring' as title
///          UNION ALL SELECT 'J.R.R. Tolkien', 'The Two Towers'
///          UNION ALL SELECT 'Andrzej Sapkowski', 'The Last Wish'
///          UNION ALL SELECT 'J.R.R. Tolkien', 'Return of the King'")
///     .fetch::<Author, _>(&client)
///     .await?;
///
/// assert_eq!(authors[0].name, "J.R.R. Tolkien");
/// assert_eq!(authors[0].books[0].title, "The Fellowship of the Ring");
/// assert_eq!(authors[0].books[1].title, "The Two Towers");
///
/// assert_eq!(authors[1].name, "Andrzej Sapkowski");
/// assert_eq!(authors[1].books[0].title, "The Last Wish");
///
/// assert_eq!(authors[2].name, "J.R.R. Tolkien");
/// assert_eq!(authors[2].books[0].title, "Return of the King");
/// # Ok(())
/// # }
/// ```
///
///
/// ### `#[row(hash)]`
///
/// Like `#[row(group)]`, but all previous rows are considered when merging. This is accomplished by
/// using a `HashMap`, hence the name. This implies that all keys have to implement the `Hash` and
/// `Eq` traits:
///
/// ```
/// # use postgres_query::*;
/// # use tokio_postgres::Client;
/// # async fn foo() -> Result<()> {
/// # let client: Client = unimplemented!();
/// #[derive(Debug, FromSqlRow)]
/// #[row(hash)]
/// struct Author {
///     #[row(key)]
///     name: String,
///
///     #[row(merge)]
///     books: Vec<Book>,
/// }
///
/// #[derive(Debug, FromSqlRow)]
/// struct Book {
///     title: String,
/// }
///
/// let authors = query!(
///         "SELECT 'J.R.R. Tolkien' as name, 'The Fellowship of the Ring' as title
///          UNION ALL SELECT 'J.R.R. Tolkien', 'The Two Towers'
///          UNION ALL SELECT 'Andrzej Sapkowski', 'The Last Wish'
///          UNION ALL SELECT 'J.R.R. Tolkien', 'Return of the King'")
///     .fetch::<Author, _>(&client)
///     .await?;
///
/// assert_eq!(authors[0].name, "J.R.R. Tolkien");
/// assert_eq!(authors[0].books[0].title, "The Fellowship of the Ring");
/// assert_eq!(authors[0].books[1].title, "The Two Towers");
/// assert_eq!(authors[0].books[2].title, "Return of the King");
///
/// assert_eq!(authors[1].name, "Andrzej Sapkowski");
/// assert_eq!(authors[1].books[0].title, "The Last Wish");
/// # Ok(())
/// # }
/// ```
///
/// ## Field attributes
///
/// These attributes are put on the fields of a container.
///
///
/// ### `#[row(rename = "...")]`
///
/// Use a name other than that of the field when looking up the name of the column.
///
/// ```
/// # use postgres_query::FromSqlRow;
/// #[derive(FromSqlRow)]
/// struct Person {
///     age: i32,
///     // matches the column named "first_name" instead of "name"
///     #[row(rename = "first_name")]
///     name: String,
/// }
/// ```
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
/// struct Customer {
///     id: i32,
///     #[row(flatten)]
///     info: Person,
/// }
///
/// #[derive(FromSqlRow)]
/// struct Person {
///     name: String,
///     age: i32
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
/// ### `#[row(stride = N)]`
///
/// Puts this field into a partition with exactly `N` columns. Only available when using the
/// `#[row(exact)]` attribute on the container,
///
/// ```
/// # use postgres_query::{FromSqlRow, query, Result};
/// # use tokio_postgres::Client;
/// # async fn foo() -> Result<()> {
/// # let client: Client = unimplemented!();
/// #[derive(Debug, FromSqlRow)]
/// struct Person {
///     id: i32,
///     name: String,
/// }
///
/// #[derive(Debug, FromSqlRow)]
/// #[row(exact)]
/// struct Family {
///     // Matches first 4 columns
///     #[row(flatten, stride = 4)]
///     parent: Person,
///     // Matches last 3 columns
///     #[row(flatten, stride = 3)]
///     child: Person,
/// }
///
/// let family = query!(
///     "SELECT
///         11 as generation,
///         1 as id, 'Bob' as name, 42 as age,
///         2 as id, 'Ike' as name, 14 as age"
///     )
///     .fetch_one::<Family, _>(&client)
///     .await?;
///     
/// assert_eq!(family.parent.id, 1);
/// assert_eq!(family.parent.name, "Bob");
/// assert_eq!(family.child.id, 2);
/// assert_eq!(family.child.name, "Ike");
/// # Ok(())
/// # }
/// ```
///
/// ### `#[row(split = "...")]`
///
/// Introduce an additional [split](extract/fn.split_columns_many.html#split-points) right
/// before this field. Requires that the container has the `split` attribute as well.
///
/// Intuitively this splits the row in two parts: every field before this attribute matches the
/// columns before the split and every field afterwards matches the second remaining columns.
///
/// ```
/// # use postgres_query::{FromSqlRow};
/// #[derive(FromSqlRow)]
/// #[row(split)]
/// struct User {
///     // `id` and `name` will only match the columns before `email`
///     id: i32,
///     name: String,
///     #[row(split = "email")]
///     // `email`, `address` and `shoe_size` will only
///     // match the columns after and including `email`
///     email: String,
///     address: String,
///     shoe_size: i32,
/// }
/// ```
///
/// Note that the first split always matches first occurence of that column. This can result in some
/// subtle bugs:
///
/// ```
/// # use postgres_query::{FromSqlRow, query};
/// #[derive(FromSqlRow)]
/// #[row(split)]
/// struct Family {
///     #[row(flatten)]
///     parent: Person,
///     #[row(flatten, split = "id")]
///     child: Person,
/// }
///
/// #[derive(FromSqlRow)]
/// struct Person {
///     name: String,
///     age: i32
/// }
///
/// let query = query!("SELECT parent.*, child.* FROM ...");
///
/// // Imagine the query above results in the following columns:
/// //
/// // Columns:                id, name, id, name
/// // Splits:                |
/// // Partitions:  +-parent-+ +-----child------+
/// ```
///
/// The split causes `parent` to match against all columns before the first `id`, ie. an empty
/// partition. This would cause an error when executing the query.
///
/// A correct split would look like this:
///
/// ```
/// # use postgres_query::{FromSqlRow, query};
/// # #[derive(FromSqlRow)] struct Person;
/// #[derive(FromSqlRow)]
/// #[row(split)]
/// struct Family {
///     #[row(flatten, split = "id")]
///     parent: Person,
///     #[row(flatten, split = "id")]
///     child: Person,
/// }
/// ```
///
///
/// ### `#[row(key)]`
///
/// Specifies this field to be a `key` field. `key` fields are compared against each other when
/// extracting values from multiple rows. Rows are merged if the key fields in each row are
/// identical. You may have multiple `key` fields within a single container, but none of them may
/// have the `#[row(merge)]` attribute. Multiple `key` fields will be treated as a tuple in
/// comparisons.
///
///
/// ### `#[row(merge)]`
///
/// Specifies this field to be a `merge` field. This requires that the field's type implements the
/// [`Merge`] trait. When two rows have been deemed to be equal based on the `key` fields, the
/// corresponding `merge` fields in those rows will be merged. You may specify multiple `merge`
/// fields within one container, but none of them may have the `#[row(key)]` attribute.
///
/// [`Merge`]: extract/trait.Merge.html
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
