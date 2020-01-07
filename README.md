
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
struct with the `#[derive(FromSqlRow)]` atttribute:

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

This crate also enables you to extract structures from rows that contain other
structures. This can be useful when you are joining two tables. Here we store
the `Author` inside of the `Book`:

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

We hove to split the row into parts by specifying that the first occurence of
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


## License

Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in rust-postgres-query by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.

