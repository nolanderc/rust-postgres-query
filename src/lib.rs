extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::*;
use std::collections::HashMap;
use syn::parse::{Parse, ParseStream};
use syn::{Ident, LitStr, Token};

struct QueryArguments {
    ident: Ident,
    sql: String,
    idents: Vec<Ident>,
}

impl Parse for QueryArguments {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident = input.parse()?;
        let _ = input.parse::<Token![,]>()?;
        let literal = input.parse()?;

        let (sql, idents) = Self::parse_literal(&literal)?;

        Ok(QueryArguments { ident, sql, idents })
    }
}

impl QueryArguments {
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
        let text = text.chars().take_while(|ch| ch.is_alphanumeric()).collect::<String>();

        let count =text.chars().count();
        let ident = syn::parse_str::<Ident>(&text)?;

        Ok((ident, count))
    }
}

#[proc_macro]
pub fn define_query(input: TokenStream) -> TokenStream {
    let args = syn::parse_macro_input!(input as QueryArguments);

    let ident = args.ident;
    let literal = args.sql;
    let idents = args.idents;

    let life_p = if idents.len() == 0 {
        None
    } else {
        Some(quote!('params))
    };

    let life_p_rep = life_p.clone().into_iter();
    let life_p_decl = quote!( #(<#life_p_rep>)* );
    let p_or_static = life_p.clone().unwrap_or_else(|| quote!('static));

    let definition = quote! {
        #[derive(Debug, Copy, Clone)]
        struct #ident #life_p_decl {
            #(#idents: &#life_p dyn ::postgres::types::ToSql,)*
        }
    };

    let param_count = idents.iter().count();
    let target_type = quote!([&#p_or_static dyn ::postgres::types::ToSql; #param_count]);

    let deref = quote! {
        impl #life_p_decl ::std::convert::Into<#target_type> for #ident #life_p_decl {
            fn into(self) -> #target_type {
                [
                    #(self.#idents,)*
                ]
            }
        }
    };

    let output = quote! {
        #definition

        impl #life_p_decl #ident #life_p_decl {
            const SQL: &'static str = #literal;
        }

        #deref
    };

    output.into()
}
