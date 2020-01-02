
# postgres-query

![Crates.io](https://img.shields.io/crates/v/postgres_query)
![Documentation](https://docs.rs/postgres_query/badge.svg)

This crate provides convenience macros and traits which help with writing SQL
queries and gathering their results into statically typed structures.

[Documentation](https://docs.rs/postgres_query)

## Example

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


## License

Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in rust-postgres-query by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.

