extern crate proc_macro;

use proc_macro2::TokenStream;
use quote::*;
use std::collections::HashMap;
use syn::parse::{Parse, ParseStream};
use syn::{braced, FieldsNamed, Ident, LitStr, Token};

struct QueryDefinition {
    queries: Vec<Query>,
}

struct Query {
    kind: QueryKind,
    ident: Ident,
    sql: String,
    idents: Vec<Ident>,
}

enum QueryKind {
    Struct,
}

impl Parse for QueryDefinition {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut queries = Vec::new();

        while !input.is_empty() {
            let query = input.parse()?;
            queries.push(query);
        }

        Ok(QueryDefinition { queries })
    }
}

impl Parse for Query {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let kind = input.parse()?;
        let ident = input.parse()?;

        let query;
        braced!(query in input);

        let literal = query.parse()?;

        let (sql, idents) = Self::parse_literal(&literal)?;

        Ok(Query { kind, ident, sql, idents })
    }
}

impl Parse for QueryKind {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();

        if lookahead.peek(Token![struct]) {
            input.parse::<Token![struct]>()?;
            Ok(QueryKind::Struct)
        } else {
            Err(lookahead.error())
        }
    }
}


impl Query {
    fn parse_literal(literal: &LitStr) -> syn::Result<(String, Vec<Ident>)> {
        let text = literal.value();

        let mut chars = text.chars();
        let mut sql = String::new();
        let mut args = HashMap::new();

        while let Some(ch) = chars.next() {
            if ch == '$' {
                let text = chars.as_str();
                if text.chars().next() == Some('$') {
                    sql.push(chars.next().unwrap());
                } else {
                    let (ident, count) = Self::parse_ident(text)?;

                    let next_arg = args.len();
                    let index = args.entry(ident).or_insert(next_arg + 1);

                    sql.push('$');
                    sql.push_str(&index.to_string());

                    for _ in 0..count {
                        chars.next().expect("Unexpected EOF");
                    }
                }
            } else {
                sql.push(ch);
            }
        }

        let mut args: Vec<_> = args.into_iter().collect();
        args.sort_by_key(|(_, index)| *index);
        let args = args.into_iter().map(|(ident, _)| ident).collect();

        Ok((sql, args))
    }

    fn parse_ident(text: &str) -> syn::Result<(Ident, usize)> {
        let text = text
            .chars()
            .take_while(|ch| ch.is_alphanumeric())
            .collect::<String>();

        let count = text.chars().count();
        let ident = syn::parse_str::<Ident>(&text)?;

        Ok((ident, count))
    }

    fn definition(self) -> TokenStream {
        let ident = self.ident;
        let literal = self.sql;
        let idents = self.idents;

        let life_p = if idents.len() == 0 {
            None
        } else {
            Some(quote!('params))
        };

        let life_p_rep = life_p.clone().into_iter();
        let life_p_decl = quote!( #(<#life_p_rep>)* );
        let p_or_static = life_p.clone().unwrap_or_else(|| quote!('static));

        let param_count = idents.iter().count();
        let target_type = quote!([&#p_or_static dyn ::postgres::types::ToSql; #param_count]);

        quote! {
            #[derive(Debug, Copy, Clone)]
            pub struct #ident #life_p_decl {
                #(#idents: &#life_p dyn ::postgres::types::ToSql,)*
            }

            impl #life_p_decl #ident #life_p_decl {
                const SQL: &'static str = #literal;

                pub fn params(self) -> #target_type {
                    [ #(self.#idents,)* ]
                }

                pub fn execute(
                    &self,
                    conn: &::postgres::Connection
                ) -> ::postgres::Result<u64> {
                    conn.execute(Self::SQL, &self.params())
                }

                pub fn query(
                    &self,
                    conn: &::postgres::Connection
                ) -> ::postgres::Result<::postgres::rows::Rows> {
                    conn.query(Self::SQL, &self.params())
                }
            }
        }
    }
}

#[proc_macro]
pub fn define_query(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let definition = syn::parse_macro_input!(input as QueryDefinition);

    let queries = definition
        .queries
        .into_iter()
        .map(|query| query.definition());

    let output = quote! {
        #( #queries )*
    };

    output.into()
}
