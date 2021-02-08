//! Abstractions over client-like types.

mod cache;

pub use cache::Caching;

use async_trait::async_trait;
use postgres_types::ToSql;
use tokio_postgres::{error::Error as SqlError, Client, RowStream, Statement, Transaction};

#[cfg(feature = "deadpool")]
use deadpool_postgres::{Client as DpClient, ClientWrapper as DpClientWrapper};


/// A generic client with basic functionality.
#[async_trait]
pub trait GenericClient {
    /// Prepare a SQL query for execution. See [`Client::prepare`] for more info.
    ///
    /// [`Client::prepare`]:
    /// https://docs.rs/tokio-postgres/0.5.1/tokio_postgres/struct.Client.html#method.prepare
    async fn prepare(&self, sql: &str) -> Result<Statement, SqlError>;

    /// Implementors may choose to override this method if they, for whatever reason (performance
    /// being one), want to cache a specific query.
    ///
    /// Because of the `'static` lifetime associated with the query string, we can assert that its
    /// value is never going to change. For instance, if a `HashMap` is used to build a cache of
    /// queries, it is enough to hash the pointer to the query instead of the whole string, since we
    /// know it will be unique for the duration of the program.
    async fn prepare_static(&self, sql: &'static str) -> Result<Statement, SqlError> {
        self.prepare(sql).await
    }

    /// Execute the given statement with the parameters specified and return the number of affected
    /// rows. See [`Client::execute_raw`] for more info.
    ///
    /// [`Client::execute_raw`]:
    /// https://docs.rs/tokio-postgres/0.5.1/tokio_postgres/struct.Client.html#method.execute_raw
    async fn execute_raw<'a>(
        &'a self,
        statement: &Statement,
        parameters: &[&'a (dyn ToSql + Sync)],
    ) -> Result<u64, SqlError>;

    /// Execute the given statement with the parameters specified and return the resulting rows as
    /// an asynchronous stream. See [`Client::query_raw`] for more info.
    ///
    /// [`Client::query_raw`]:
    /// https://docs.rs/tokio-postgres/0.5.1/tokio_postgres/struct.Client.html#method.query_raw
    async fn query_raw<'a>(
        &'a self,
        statement: &Statement,
        parameters: &[&'a (dyn ToSql + Sync)],
    ) -> Result<RowStream, SqlError>;
}

fn slice_iter<'a>(
    s: &'a [&'a (dyn ToSql + Sync)],
) -> impl ExactSizeIterator<Item = &'a dyn ToSql> + 'a {
    s.iter().map(|s| *s as _)
}

#[async_trait]
impl GenericClient for Client {
    #[deny(unconditional_recursion)]
    async fn prepare(&self, sql: &str) -> Result<Statement, SqlError> {
        Client::prepare(self, sql).await
    }

    #[deny(unconditional_recursion)]
    async fn execute_raw<'a>(
        &'a self,
        statement: &Statement,
        parameters: &[&'a (dyn ToSql + Sync)],
    ) -> Result<u64, SqlError> {
        Client::execute_raw(self, statement, slice_iter(parameters)).await
    }

    #[deny(unconditional_recursion)]
    async fn query_raw<'a>(
        &'a self,
        statement: &Statement,
        parameters: &[&'a (dyn ToSql + Sync)],
    ) -> Result<RowStream, SqlError> {
        Client::query_raw(self, statement, slice_iter(parameters)).await
    }
}

#[cfg(feature = "deadpool")]
#[async_trait]
impl GenericClient for DpClient {
    #[deny(unconditional_recursion)]
    async fn prepare(&self, sql: &str) -> Result<Statement, SqlError> {
        DpClientWrapper::prepare(self, sql).await
    }

    #[deny(unconditional_recursion)]
    async fn execute_raw<'a>(
        &'a self,
        statement: &Statement,
        parameters: &[&'a (dyn ToSql + Sync)],
    ) -> Result<u64, SqlError> {
        Client::execute_raw(&*self, statement, slice_iter(parameters)).await
    }

    #[deny(unconditional_recursion)]
    async fn query_raw<'a>(
        &'a self,
        statement: &Statement,
        parameters: &[&'a (dyn ToSql + Sync)],
    ) -> Result<RowStream, SqlError> {
        Client::query_raw(&*self, statement, slice_iter(parameters)).await
    }
}

#[async_trait]
impl GenericClient for Transaction<'_> {
    async fn prepare(&self, sql: &str) -> Result<Statement, SqlError> {
        Transaction::prepare(self, sql).await
    }

    async fn execute_raw<'a>(
        &'a self,
        statement: &Statement,
        parameters: &[&'a (dyn ToSql + Sync)],
    ) -> Result<u64, SqlError> {
        Transaction::execute_raw::<_, _, Statement>(self, statement, slice_iter(parameters)).await
    }

    async fn query_raw<'a>(
        &'a self,
        statement: &Statement,
        parameters: &[&'a (dyn ToSql + Sync)],
    ) -> Result<RowStream, SqlError> {
        Transaction::query_raw(self, statement, slice_iter(parameters)).await
    }
}

macro_rules! client_deref_impl {
    ($($target:tt)+) => {
        #[async_trait]
        impl<T> GenericClient for $($target)+ where T: GenericClient + Sync {
            async fn prepare(&self, sql: &str) -> Result<Statement, SqlError> {
                T::prepare(self, sql).await
            }

            async fn execute_raw<'a>(
                &'a self,
                statement: &Statement,
                parameters: &[&'a (dyn ToSql + Sync)],
            ) -> Result<u64, SqlError> {
                T::execute_raw(self, statement, parameters).await
            }

            async fn query_raw<'a>(
                &'a self,
                statement: &Statement,
                parameters: &[&'a (dyn ToSql + Sync)],
            ) -> Result<RowStream, SqlError> {
                T::query_raw(self, statement, parameters).await
            }
        }
    }
}

client_deref_impl!(&T);
