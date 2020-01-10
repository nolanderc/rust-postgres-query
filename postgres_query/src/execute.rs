//! Executing queries through a client.
//!
//! See [`Query`].
//!
//! [`Query`]: ../struct.Query.html

use super::{Query, Sql};
use crate::client::GenericClient;
use crate::error::Result;
use crate::extract::{self, FromSqlRow};
use futures::{pin_mut, Stream, StreamExt, TryStreamExt};
use thiserror::Error;
use tokio_postgres::{error::Error as SqlError, Row, Statement};

/// An error that may arise when executing a query.
#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to execute query")]
    Sql(#[from] SqlError),

    #[error("expected 1 row, found 0")]
    NoRows,
    #[error("expected 1 row, found more than 1")]
    TooManyRows,

    #[error("failed to extract value from row")]
    Extract(#[from] extract::Error),
}

impl<'a> Query<'a> {
    /// Execute this query and return the number of affected rows.
    pub async fn execute<C>(&self, client: &C) -> Result<u64>
    where
        C: GenericClient + Sync,
    {
        let statement = self.prepare(&client).await?;
        let rows = client
            .execute_raw(&statement, &self.parameters)
            .await
            .map_err(Error::from)?;
        Ok(rows)
    }

    /// Execute this query and return the resulting values.
    pub async fn fetch<T, C>(&self, client: &C) -> Result<Vec<T>>
    where
        T: FromSqlRow,
        C: GenericClient + Sync,
    {
        let rows = self.query(client).await?;
        let values = T::from_row_multi(&rows).map_err(Error::from)?;
        Ok(values)
    }

    /// Execute this query and return the resulting value. This method will return an error if, not
    /// exactly one row was returned by the query.
    pub async fn fetch_one<T, C>(&self, client: &C) -> Result<T>
    where
        T: FromSqlRow,
        C: GenericClient + Sync,
    {
        let row = self.query_one(client).await?;
        dbg!(&row.columns());
        let value = T::from_row(&row).map_err(Error::from)?;
        Ok(value)
    }

    /// Execute this query and return the resulting values as an asynchronous stream of values.
    pub async fn fetch_streaming<T, C>(&self, client: &C) -> Result<impl Stream<Item = Result<T>>>
    where
        T: FromSqlRow,
        C: GenericClient + Sync,
    {
        let rows = self.query_streaming(client).await?;
        let values = rows.map(|row| {
            row.and_then(|row| {
                T::from_row(&row)
                    .map_err(Error::Extract)
                    .map_err(Into::into)
            })
        });
        Ok(values)
    }

    /// Execute this query and return the resulting rows.
    pub async fn query<C>(&self, client: &C) -> Result<Vec<Row>>
    where
        C: GenericClient + Sync,
    {
        let statement = self.prepare(&client).await?;
        let rows = client
            .query_raw(&statement, &self.parameters)
            .await
            .map_err(Error::from)?
            .try_collect::<Vec<_>>()
            .await
            .map_err(Error::from)?;
        Ok(rows)
    }

    /// Execute this query and return the resulting row. This method will return an error if, not
    /// exactly one row was returned by the query.
    pub async fn query_one<C>(&self, client: &C) -> Result<Row>
    where
        C: GenericClient + Sync,
    {
        let statement = self.prepare(&client).await?;
        let rows = client
            .query_raw(&statement, &self.parameters)
            .await
            .map_err(Error::from)?;

        pin_mut!(rows);

        let row = match rows.try_next().await.map_err(Error::from)? {
            Some(row) => row,
            None => return Err(Error::NoRows.into()),
        };

        if rows.try_next().await.map_err(Error::from)?.is_some() {
            return Err(Error::TooManyRows.into());
        }

        Ok(row)
    }

    /// Execute this query and return the resulting values as an asynchronous stream of values.
    pub async fn query_streaming<C>(&self, client: &C) -> Result<impl Stream<Item = Result<Row>>>
    where
        C: GenericClient + Sync,
    {
        let statement = self.prepare(&client).await?;
        let rows = client
            .query_raw(&statement, &self.parameters)
            .await
            .map_err(Error::from)?;
        Ok(rows.map_err(Error::from).map_err(Into::into))
    }
}

impl<'a> Query<'a> {
    async fn prepare<C>(&self, client: &C) -> Result<Statement>
    where
        C: GenericClient + Sync,
    {
        let result = match &self.sql {
            Sql::Static(text) => client.prepare_static(text).await,
            Sql::Dynamic(text) => client.prepare(&text).await,
        };

        result.map_err(Error::Sql).map_err(Into::into)
    }
}
