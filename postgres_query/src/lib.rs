pub use postgres_query_derive::*;

use postgres::rows::Rows;
use postgres::types::ToSql;
use postgres::GenericConnection;

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

