
# postgres-query

[![Crates.io](https://img.shields.io/crates/v/postgres_query)](https://crates.io/crates/postgres-query)
[![License](https://img.shields.io/crates/l/postgres_query)](#license)
[![Minimum Rust Version](https://img.shields.io/badge/rustc-1.40%2B-orange)](https://www.rust-lang.org/)
[![Documentation](https://docs.rs/postgres_query/badge.svg)](https://docs.rs/postgres_query)

This crate provides convenience macros and traits which help with writing SQL
queries and gathering their results into statically typed structures.

[Documentation](https://docs.rs/postgres_query)


# Example

```rust
// Connect to the database
let client: Client = connect(/* ... */);

// Construct the query
let query = query!(
    "SELECT name, age FROM people WHERE age >= $min_age",
    min_age = 18
);

// Define the structure of the data returned from the query
#[derive(FromSqlRow)]
struct Person {
    age: i32,
    name: String,
}

// Execute the query
let people: Vec<Person> = query.fetch(&client).await?;

// Use the results
for person in people {
    println!("{} is {} years young", person.name, person.age);
}
```

# Features


## Extractors

This crate allows you to extract the result of queries simply by tagging a
struct with the `#[derive(FromSqlRow)]` attribute:

```rust
#[derive(FromSqlRow)]
struct Book {
    id: i32,
    title: String,
    genre: String,
}

let books: Vec<Book> = query!("SELECT * FROM books")
    .fetch(&client)
    .await?;
```


## Multi-mapping

You may also extract multiple structures from a single row. This can be useful
when you are joining two tables. As a motivating example, we can store an
`Author` instance inside a `Book` instance, which can be easier to work with:

```rust
#[derive(FromSqlRow)]
#[row(split)]
struct Book {
    #[row(split = "id")]
    id: i32,
    title: String,
    genre: String,

    #[row(flatten, split = "id")]
    author: Author,
}

#[derive(FromSqlRow)]
struct Author {
    id: i32,
    name: String,
    birthyear: i32,
}

let books: Vec<Book> = query!(
        "SELECT books.*, authors.* 
        FROM books
        INNER JOIN authors ON authors.id = books.id"
    )
    .fetch(&client)
    .await?;
```

We have to split the row into parts by specifying that the first occurrence of
`id` is part of the book and the second `id` part of the author. The rest is
done for you.

If we wanted to reuse an already existing `Book` we could just as easily do
the following:

```rust
#[derive(FromSqlRow)]
#[row(split)]
struct Listings {
    #[row(flatten, split = "id")]
    book: Book
    #[row(flatten, split = "id")]
    author: Author,
}
```


## Dynamic queries

Queries may be constructed from either a string literal, in which case parameter
bindings are computed at compile time, or any other `String` dynamically at
runtime. The same is true for parameter bindings, which in the latter case can
be added dynamically.

Let's say that we wanted to dynamically add filters to our query:

```rust
// We have the query we want to execute
let mut sql = "SELECT * FROM people".to_string();

// and some filters we got from the user.
let age_filter: Option<i32> = Some(32);
let name_filter: Option<&str> = None;

// Then we dynamically build a list of filters and bindings to use:
let mut filters = Vec::new();
let mut bindings = Vec::new();

// We add the filters as needed.
if let Some(age) = age_filter.as_ref() {
    filters.push("age > $min_age");
    bindings.push(("min_age", age as Parameter));
}

if let Some(name) = name_filter.as_ref() {
    filters.push("name LIKE $name");
    bindings.push(("name", name as Parameter));
}

// And append them to the query.
if filters.len() > 0 {
    sql += &format!(" WHERE {}", filters.join(" AND "));
}

// Then we can use it as normal.
let query: Query = query_dyn!(&sql, ..bindings)?;
```


## License

Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in rust-postgres-query by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.

