pub use postgres_query_macro::FromSqlRow;

use super::Query;
use crate::client::GenericClient;

use futures::{Stream, StreamExt, TryStreamExt};
use tokio_postgres::{error::Error as SqlError, Column, Row};

#[derive(Debug)]
pub enum Error {
    Custom(Box<dyn std::error::Error>),
    Sql(SqlError),
}

pub trait FromSqlRow {
    fn from_row(row: Row) -> Result<Self, Error>
    where
        Self: Sized;

    fn from_row_multi(columns: &[Column], rows: Vec<Row>) -> Result<Vec<Self>, Error>
    where
        Self: Sized,
    {
        let _ = columns;
        rows.into_iter().map(Self::from_row).collect()
    }
}

impl<'a> Query<'a> {
    pub async fn execute(&self, client: &impl GenericClient) -> Result<u64, Error> {
        let statement = client.prepare(self.sql).await?;
        let rows = client.execute_raw(&statement, &self.parameters).await?;
        Ok(rows)
    }

    pub async fn fetch<T, C>(&self, client: &C) -> Result<Vec<T>, Error>
    where
        T: FromSqlRow,
        C: GenericClient,
    {
        let statement = client.prepare(self.sql).await?;
        let rows = client
            .query_raw(&statement, &self.parameters)
            .await?
            .try_collect::<Vec<_>>()
            .await?;

        T::from_row_multi(statement.columns(), rows)
    }

    pub async fn fetch_streaming<T, C>(
        &self,
        client: &C,
    ) -> Result<impl Stream<Item = Result<T, Error>>, Error>
    where
        T: FromSqlRow,
        C: GenericClient,
    {
        let statement = client.prepare(self.sql).await?;
        let rows = client.query_raw(&statement, &self.parameters).await?;

        let rows = rows.map(|row| row.map_err(Error::Sql).and_then(T::from_row));

        Ok(rows)
    }
}

impl From<SqlError> for Error {
    fn from(error: SqlError) -> Self {
        Error::Sql(error)
    }
}
