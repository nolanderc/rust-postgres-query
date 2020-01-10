use proc_macro2::{Span, TokenStream};
use quote::*;
use syn::{
    token::{Enum, Union},
    Data, DataEnum, DataUnion, DeriveInput, Error, Field, Fields, Ident, Result,
};

pub fn derive(input: DeriveInput) -> TokenStream {
    let lib = quote! { postgres_query };

    let ident = &input.ident;

    let columns = match extract_columns(&input) {
        Ok(columns) => columns,
        Err(e) => return e.to_compile_error(),
    };

    let mut idents = Vec::new();
    let getters = columns
        .into_iter()
        .enumerate()
        .map(|(i, column)| {
            let index = match column.index {
                Index::Position => quote! { #i },
                Index::Name(name) => {
                    let name = name.to_string();
                    quote! { #name }
                }
            };

            let field = column
                .field
                .ident
                .unwrap_or_else(|| Ident::new(&format!("column_{}", i), Span::call_site()));
            let ty = column.field.ty;

            idents.push(field.clone());

            quote! {
                let #field = row.try_get::<_, #ty>(#index)?;
            }
        })
        .collect::<TokenStream>();

    let constructor = make_constructor(&input, idents);

    quote! {
        impl #lib::FromSqlRow for #ident {
            fn from_row<R>(row: &R) -> Result<Self, #lib::extract::Error>
            where
                R: #lib::extract::Row
            {
                #getters
                Ok(#constructor)
            }
        }
    }
}

struct Column {
    index: Index,
    field: Field,
}

enum Index {
    Position,
    Name(Ident),
}

fn extract_columns(input: &DeriveInput) -> Result<Vec<Column>> {
    match &input.data {
        Data::Struct(data) => {
            let columns = data
                .fields
                .iter()
                .map(|field| {
                    let index = match &field.ident {
                        None => Index::Position,
                        Some(name) => Index::Name(name.clone()),
                    };
                    Column {
                        index,
                        field: field.clone(),
                    }
                })
                .collect();

            Ok(columns)
        }
        Data::Enum(DataEnum {
            enum_token: Enum { span },
            ..
        })
        | Data::Union(DataUnion {
            union_token: Union { span, .. },
            ..
        }) => Err(Error::new(
            *span,
            "`FromSqlRow` may only be derived for `struct`s",
        )),
    }
}

fn make_constructor(input: &DeriveInput, locals: impl IntoIterator<Item = Ident>) -> TokenStream {
    let ident = &input.ident;

    let mut fields = TokenStream::new();
    fields.append_separated(locals, quote! { , });

    match &input.data {
        Data::Struct(data) => match data.fields {
            Fields::Unnamed(_) => quote! { #ident ( #fields ) },
            Fields::Named(_) => quote! { #ident { #fields } },
            Fields::Unit => {
                if fields.is_empty() {
                    quote! { #ident }
                } else {
                    panic!("Attempted to construct unit struct with fields");
                }
            }
        },
        _ => panic!(),
    }
}
