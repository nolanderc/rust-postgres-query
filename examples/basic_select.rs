use postgres_query::define_query;
use postgres::{Connection, TlsMode};

define_query! {
    struct CreatePerson {
        "CREATE TABLE IF NOT EXISTS person (
             id              SERIAL PRIMARY KEY,
             name            VARCHAR NOT NULL,
             age             INTEGER
         )"
    }

    struct InsertPerson {
        "INSERT INTO person (name, age) VALUES ($name, $age)"
    }

    struct NameQuery {
        "SELECT id, name, age FROM person WHERE name = $name"
    }
}

fn main() {
    let conn = Connection::connect("postgres://postgres@localhost:5432", TlsMode::None).unwrap();

    let create = CreatePerson {};
    create.execute(&conn).unwrap();

    let insert = InsertPerson {
        name: &"Jake",
        age: &23,
    };

    insert.execute(&conn).unwrap();

    let name = NameQuery {
        name: &"Jake",
    };

    for row in &name.query(&conn).unwrap() {
        let id: i32 = row.get("id");
        let name: String = row.get("name");
        let age: i32 = row.get("age");

        println!("Found person {}: {} age {}", id, name, age);
    }
}

