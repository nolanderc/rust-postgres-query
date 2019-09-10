//!
//! Exposes the trait `Query` which can be used to execute queries against a 
//! [rust-postgres](https://docs.rs/postgres/0.15.2/postgres/index.html)
//! database connection.
//!
//! The trait `Query` may be derived and used as follows:
//! ```
//! # use postgres_query_derive::*;
//! # use postgres_query::*;
//! # use postgres::{Connection, TlsMode};
//! # let connection = Connection::connect("postgres://postgres@localhost:5432", TlsMode::None).unwrap();
//! # connection.execute("CREATE TABLE IF NOT EXISTS people ( name TEXT, age INTEGER )", &[]).unwrap();
//! #[derive(Query)]
//! #[query(sql = "SELECT * FROM people WHERE name = $name AND age >= $min_age")]
//! struct PersonByName {
//!     name: String,
//!     min_age: i32,
//! }
//!
//! let get_person = PersonByName {
//!     name: "Josh".to_owned(),
//!     min_age: 19,
//! };
//!
//! let rows = get_person.query(&connection).unwrap();
//! ```
//!


pub use postgres_query_derive::*;

use postgres::rows::Rows;
use postgres::types::ToSql;
use postgres::GenericConnection;

/// A type which can execute an SQL query.
pub trait Query<'a> {
    type Sql: AsRef<str>;
    type Params: AsRef<[&'a dyn ToSql]>;

    /// Get the SQL query for this type.
    fn sql(&'a self) -> Self::Sql;

    /// Get the SQL parameters for this type.
    fn params(&'a self) -> Self::Params;

    /// Execute this query and return the number of affected rows.
    fn execute<C>(&'a self, connection: &C) -> postgres::Result<u64>
    where
        C: GenericConnection,
    {
        connection.execute(self.sql().as_ref(), self.params().as_ref())
    }

    /// Execute this query and return the resulting rows.
    fn query<C>(&'a self, connection: &C) -> postgres::Result<Rows>
    where
        C: GenericConnection,
    {
        connection.query(self.sql().as_ref(), self.params().as_ref())
    }
}

