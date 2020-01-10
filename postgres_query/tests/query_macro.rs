use bytes::BytesMut;
use postgres_query::*;
use postgres_types::{IsNull, ToSql, Type};

#[test]
fn text_only() {
    let query = query!("SELECT id, name FROM people");

    assert_eq!(query.sql(), "SELECT id, name FROM people");
    assert_params_eq(query.parameters(), &[])
}

#[test]
fn escape_dollar() {
    let query = query!("SELECT $$");
    assert_eq!(query.sql(), "SELECT $");
    assert_params_eq(query.parameters(), &[])
}

#[test]
fn parameter_substitution_implicit_name() {
    let age = 42;
    let query = query!("SELECT id, name FROM people WHERE age = $age", age);

    assert_eq!(query.sql(), "SELECT id, name FROM people WHERE age = $1");
    assert_params_eq(query.parameters(), &[(&age, &Type::INT4)])
}

#[test]
fn parameter_substitution_explicit_name() {
    let query = query!("SELECT id, name FROM people WHERE age = $age", age = 42);

    assert_eq!(query.sql(), "SELECT id, name FROM people WHERE age = $1");
    assert_params_eq(query.parameters(), &[(&42, &Type::INT4)])
}

#[test]
fn parameter_substitution_multiple_parameters() {
    let query = query!("$a $b $c", a = 42, b = "John Wick", c = Option::<i32>::None,);

    assert_eq!(query.sql(), "$1 $2 $3");
    assert_params_eq(
        query.parameters(),
        &[
            (&42, &Type::INT4),
            (&"John Wick", &Type::TEXT),
            (&Option::<i32>::None, &Type::INT4),
        ],
    )
}

#[test]
fn dynamic_query() {
    let filters = ["age > $min_age", "name LIKE $name"].join(" AND ");

    let query = query_dyn!(
        &format!("SELECT * FROM people WHERE {}", filters),
        min_age = 32,
        name = "%John%",
    )
    .unwrap();

    assert_eq!(
        query.sql(),
        "SELECT * FROM people WHERE age > $1 AND name LIKE $2"
    );
}

#[test]
fn dynamic_query_dynamic_bindings() -> Result<()> {
    let mut filters = Vec::new();
    let mut bindings = Vec::<(&str, Parameter)>::new();

    filters.push("age > $min_age");
    bindings.push(("min_age", &32));

    filters.push("name LIKE $name");
    bindings.push(("name", &"%John%"));

    let filters = filters.join(" AND ");
    let sql = format!("SELECT * FROM people WHERE {}", filters);

    let query = query_dyn!(&sql, ..bindings).unwrap();

    assert_eq!(
        query.sql(),
        "SELECT * FROM people WHERE age > $1 AND name LIKE $2"
    );

    assert_params_eq(
        query.parameters(),
        &[(&32, &Type::INT4), (&"%John%", &Type::TEXT)],
    );

    Ok(())
}

fn assert_params_eq<'a>(a: &[&'a (dyn ToSql + Sync)], b: &[(&'a dyn ToSql, &'a Type)]) {
    assert_eq!(a.len(), b.len());
    for (a, (b, ty)) in a.iter().copied().zip(b.iter().copied()) {
        sql_eq(a, b, ty);
    }
}

/// Check if two SQL values are of the same type and value
fn sql_eq(a: &dyn ToSql, b: &dyn ToSql, ty: &Type) -> bool {
    let mut a_buffer = BytesMut::new();
    let mut b_buffer = BytesMut::new();

    let a_result = a.to_sql_checked(ty, &mut a_buffer);
    let b_result = b.to_sql_checked(ty, &mut b_buffer);

    let is_null = |null| match null {
        IsNull::Yes => true,
        IsNull::No => false,
    };

    a_result.is_ok()
        && b_result.is_ok()
        && is_null(a_result.unwrap()) == is_null(b_result.unwrap())
        && a_buffer == b_buffer
}
