use postgres_query::Query;
use postgres::{Connection, TlsMode, types::ToSql};

#[derive(Query)]
#[query(sql = "
    CREATE TABLE IF NOT EXISTS person (
         id              SERIAL PRIMARY KEY,
         name            VARCHAR NOT NULL,
         age             INTEGER
     )
")]
struct CreatePerson;

#[derive(Query)]
#[query(sql = "INSERT INTO person (name, age) VALUES ($name, $age)")]
struct InsertPerson<'a> {
    name: &'a str,
    age: Option<i32>,
}

#[derive(Query)]
#[query(sql = "SELECT id, name, age FROM person WHERE name = $first_name || ' ' || $last_name")]
struct NameQuery<'a> {
    first_name: &'a dyn ToSql,
    last_name: &'a str,
}

fn main() {
    let conn = Connection::connect("postgres://postgres@localhost:5432", TlsMode::None).unwrap();

    let create = CreatePerson;
    create.execute(&conn).unwrap();

    let insert = InsertPerson {
        name: "Cave Johnson",
        age: Some(23),
    };

    insert.execute(&conn).unwrap();

    let name = NameQuery {
        first_name: &"Cave",
        last_name: "Johnson"
    };

    for row in &name.query(&conn).unwrap() {
        let id: i32 = row.get("id");
        let name: String = row.get("name");
        let age: i32 = row.get("age");

        println!("Found person {}: {} age {}", id, name, age);
    }
}

