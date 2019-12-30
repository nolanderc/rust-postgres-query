use async_trait::async_trait;
use postgres_types::ToSql;
use tokio_postgres::{error::Error as SqlError, Client, RowStream, Statement, Transaction};

#[async_trait(?Send)]
pub trait GenericClient {
    async fn prepare(&self, sql: &str) -> Result<Statement, SqlError>;

    async fn execute_raw<'a>(
        &'a self,
        statement: &Statement,
        parameters: &[&'a dyn ToSql],
    ) -> Result<u64, SqlError>;

    async fn query_raw<'a>(
        &'a self,
        statement: &Statement,
        parameters: &[&'a dyn ToSql],
    ) -> Result<RowStream, SqlError>;
}

#[async_trait(?Send)]
impl GenericClient for Client {
    async fn prepare(&self, sql: &str) -> Result<Statement, SqlError> {
        Client::prepare(self, sql).await
    }

    async fn execute_raw<'a>(
        &'a self,
        statement: &Statement,
        parameters: &[&'a dyn ToSql],
    ) -> Result<u64, SqlError> {
        Client::execute_raw(self, statement, parameters.iter().copied()).await
    }

    async fn query_raw<'a>(
        &'a self,
        statement: &Statement,
        parameters: &[&'a dyn ToSql],
    ) -> Result<RowStream, SqlError> {
        Client::query_raw(self, statement, parameters.iter().copied()).await
    }
}

#[async_trait(?Send)]
impl GenericClient for Transaction<'_> {
    async fn prepare(&self, sql: &str) -> Result<Statement, SqlError> {
        Transaction::prepare(self, sql).await
    }

    async fn execute_raw<'a>(
        &'a self,
        statement: &Statement,
        parameters: &[&'a dyn ToSql],
    ) -> Result<u64, SqlError> {
        Transaction::execute_raw::<_, Statement>(self, statement, parameters.iter().copied()).await
    }

    async fn query_raw<'a>(
        &'a self,
        statement: &Statement,
        parameters: &[&'a dyn ToSql],
    ) -> Result<RowStream, SqlError> {
        Transaction::query_raw(self, statement, parameters.iter().copied()).await
    }
}

macro_rules! client_deref_impl {
    ($($target:tt)+) => {
        #[async_trait(?Send)]
        impl<T> GenericClient for $($target)+ where T: GenericClient {
            async fn prepare(&self, sql: &str) -> Result<Statement, SqlError> {
                T::prepare(self, sql).await
            }

            async fn execute_raw<'a>(
                &'a self,
                statement: &Statement,
                parameters: &[&'a dyn ToSql],
            ) -> Result<u64, SqlError> {
                T::execute_raw(self, statement, parameters).await
            }

            async fn query_raw<'a>(
                &'a self,
                statement: &Statement,
                parameters: &[&'a dyn ToSql],
            ) -> Result<RowStream, SqlError> {
                T::query_raw(self, statement, parameters).await
            }
        }
    }
}

client_deref_impl!(&T);
client_deref_impl!(Box<T>);

