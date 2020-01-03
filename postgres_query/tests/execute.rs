//! Validate that queries are executed as intended.
//!
//!
//! # Setup
//!
//! These tests require access to a PostgreSQL database. To run these tests it is recommended that
//! you create a new user that has access to an empty database. By default these tests assume a user
//! with the name `postgres_query_test`. Then, initialize the environment variable
//! `POSTGRES_DB_CONFIG` to point to this new user (this variable uses the same format as
//! `tokio_postgres::connect`).

use anyhow::{anyhow, Error};
use postgres_query::{client::Caching, query, FromSqlRow};
use std::env;
use tokio_postgres::Client;

type Result<T = (), E = Error> = std::result::Result<T, E>;

/// Establish a new connection to the database
async fn establish() -> Result<Client> {
    let config = env::var("POSTGRES_DB_CONFIG")
        .unwrap_or_else(|_| "user=postgres_query_test host=localhost".to_owned());
    let (client, conn) = tokio_postgres::connect(&config, tokio_postgres::NoTls)
        .await
        .map_err(|e| {
            anyhow!(
                "failed to establish connection to database \
                 (have you set the POSTGRES_DB_CONFIG environment variable?): {}",
                e
            )
        })?;

    tokio::spawn(async move {
        conn.await.unwrap();
    });

    Ok(client)
}

#[tokio::test]
async fn simple_select() -> Result {
    let client = establish().await?;

    let query = query!("SELECT 14");
    let row = query.query_one(&client).await.unwrap();
    let value: i32 = row.get(0);

    assert_eq!(value, 14);
    Ok(())
}

#[tokio::test]
async fn simple_select_fetch() -> Result {
    let client = establish().await?;

    let value: (i32,) = query!("SELECT 14").fetch_one(&client).await.unwrap();

    assert_eq!(value, (14,));
    Ok(())
}

#[tokio::test]
async fn cached_fetch() -> Result {
    let client = establish().await?;
    let client = Caching::new(client);

    for _ in 0..10 {
        let query = query!("SELECT 'Myke', 31");
        let (name, age): (String, i32) = query.fetch_one(&client).await.unwrap();

        assert_eq!(name, "Myke");
        assert_eq!(age, 31);
    }
    Ok(())
}

#[tokio::test]
async fn fetch_named_struct() -> Result {
    let client = establish().await?;

    #[derive(FromSqlRow)]
    struct Person {
        age: i32,
        name: String,
    }

    let query = query!("SELECT 'Myke' as name, 31 as age");
    let person: Person = query.fetch_one(&client).await.unwrap();

    assert_eq!(person.name, "Myke");
    assert_eq!(person.age, 31);
    Ok(())
}

#[tokio::test]
async fn cached_transaction() -> Result {
    let client = establish().await?;
    let mut client = Caching::new(client);

    let tx: Caching<_> = client.transaction().await?;

    tx.into_inner().rollback().await?;

    Ok(())
}

#[tokio::test]
async fn fetch_joined_relations() -> Result {
    let mut client = establish().await?;
    let tx = client.transaction().await?;

    query!(
        "CREATE TABLE orders (
            id SERIAL PRIMARY KEY,
            customer TEXT
        )"
    )
    .execute(&tx)
    .await?;

    query!(
        "CREATE TABLE order_items (
            order_id INTEGER REFERENCES orders(id),
            item TEXT NOT NULL
        )"
    )
    .execute(&tx)
    .await?;

    #[derive(FromSqlRow)]
    struct OrderId(i32);

    let orders = query!(
        "INSERT INTO orders (customer) 
        VALUES 
            ('Emma'), 
            ('Anna')
        RETURNING id",
    )
    .fetch::<OrderId, _>(&tx)
    .await?;

    query!(
        "INSERT INTO order_items (order_id, item)
        VALUES 
            ($emma, 'Hair dryer'), 
            ($emma, 'Phone'), 
            ($anna, 'Note book')",
        emma = orders[0].0,
        anna = orders[1].0,
    )
    .execute(&tx)
    .await?;

    #[derive(Debug, PartialEq, FromSqlRow)]
    struct Order {
        customer: String,
        item: String,
    }

    let orders = query!(
        "SELECT 
            customer, 
            item
        FROM order_items
        INNER JOIN orders ON order_items.order_id = orders.id
        ORDER BY customer, item"
    )
    .fetch::<Order, _>(&tx)
    .await?;

    assert_eq!(orders.len(), 3);

    assert_eq!(orders[0].customer, "Anna");
    assert_eq!(orders[0].item, "Note book");

    assert_eq!(orders[1].customer, "Emma");
    assert_eq!(orders[1].item, "Hair dryer");

    assert_eq!(orders[2].customer, "Emma");
    assert_eq!(orders[2].item, "Phone");

    Ok(())
}
