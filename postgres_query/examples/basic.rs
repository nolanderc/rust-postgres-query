use anyhow::Error;
use postgres_query::{query, FromSqlRow};
use structopt::StructOpt;
use tokio_postgres::{config::Config, NoTls};

#[derive(StructOpt)]
struct Options {
    /// The database configuration, given as a string of space separated key-value pairs (eg.
    /// 'host=localhost user=postgres').
    config: Vec<String>,
}

#[derive(FromSqlRow)]
struct Person {
    name: String,
    age: i32,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let options = Options::from_args();
    let config: Config = options.config.join(" ").parse()?;

    let (mut client, connection) = config.connect(NoTls).await?;

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    // open a new transaction to avoid making any changes to the database
    let tx = client.transaction().await?;

    query!("CREATE TABLE people (name TEXT, age INT)")
        .execute(&tx)
        .await?;

    query!(
        "INSERT INTO people VALUES ($name, $age)",
        name = "John Wick",
        age = 42,
    )
    .execute(&tx)
    .await?;

    let query = query!("SELECT name, age FROM people");
    let people: Vec<Person> = query.fetch(&tx).await?;

    for person in people {
        println!("{} is {} years young", person.name, person.age);
    }

    // undo any changes
    tx.rollback().await?;
    Ok(())
}
