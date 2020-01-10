extern crate proc_macro;

#[macro_use]
mod macros;

mod query;
mod from_sql_row;

use proc_macro::TokenStream;
use proc_macro_hack::proc_macro_hack;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_hack]
pub fn query(input: TokenStream) -> TokenStream {
    let query = parse_macro_input!(input as query::QueryInput);
    let output = query.convert_to_struct();
    TokenStream::from(output)
}

#[proc_macro_derive(FromSqlRow, attributes(row))]
pub fn from_sql_row(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let output = from_sql_row::derive(input);
    TokenStream::from(output)
}

