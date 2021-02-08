#![cfg(feature = "deadpool")]

use postgres_query::*;
use deadpool_postgres::{Pool, Client, Config};

fn connect() -> Pool {
    let mut cfg = Config::new();
    cfg.dbname = Some("postgres_query_test".to_string());
    cfg.host = Some("localhost".to_string());
    cfg.create_pool(tokio_postgres::NoTls).unwrap()
}

#[tokio::test]
async fn simple_query() {
    let pool = connect();
    let client: Client = pool.get().await.unwrap();
    let query: Query = query_dyn!("SELECT 14").unwrap();
    let res = query.fetch_one::<(i32,), _>(&client).await;
}
