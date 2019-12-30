pub mod client;
mod fetch;

pub use fetch::{Error, FromSqlRow};

use postgres_types::ToSql;
use proc_macro_hack::proc_macro_hack;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[proc_macro_hack]
pub use postgres_query_macro::query;

/// A query
#[derive(Debug, Clone)]
pub struct Query<'a> {
    pub sql: &'static str,
    pub parameters: Box<[&'a dyn ToSql]>,
}

#[cfg(test)]
fn test() {
    let query = query!(
        "$a $b $c",
        a = 42,
        b = "John Wick",
        c = Option::<i32>::None,
    );
}

