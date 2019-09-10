use postgres::types::ToSql;
use postgres_query::Query;

#[derive(Query)]
#[query(sql = "SELECT * FROM users WHERE age > $age AND name > $name")]
struct AgeQuery<'a, T>
where
    T: ToSql,
{
    name: &'a T,
    age: i32,
}

#[test]
fn derive_query_basic() {
    let name = &"Jake";
    let age = 21;

    let query = AgeQuery { age, name };

    let params = query.params();
    let expected_params: &[&dyn ToSql] = &[&age, &name];

    assert_eq!(format!("{:?}", params), format!("{:?}", expected_params));

    assert_eq!(
        query.sql(),
        "SELECT * FROM users WHERE age > $1 AND name > $2",
    );
}
