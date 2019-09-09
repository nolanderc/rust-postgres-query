
use postgres_query::*;

define_query!(Query, "SELECT id FROM users WHERE name = $name");

#[test]
fn define_query_basic() {
    let name = "Jake";

    let query = Query {
        name: &name,
        word: &name,
    };

    assert_eq!(
        Query::SQL, 
        "SELECT id FROM users WHERE name = $1",
    );
}
