use proc_macro2::{Span, TokenStream};
use quote::*;
use syn::{
    spanned::Spanned,
    token::{Enum, Union},
    Attribute, Data, DataEnum, DataUnion, DeriveInput, Error, Field, Fields, Ident, Lit, Meta,
    NestedMeta, Result,
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
            let ident = column
                .field
                .ident
                .unwrap_or_else(|| Ident::new(&format!("column_{}", i), Span::call_site()));
            let ty = column.field.ty;

            idents.push(ident.clone());

            let getter = match column.index {
                Index::Position => {
                    quote! {
                        row.try_get(#i)?
                    }
                }
                Index::Name(name) => {
                    let column = name.to_string();
                    quote! {
                        row.try_get(#column)?
                    }
                }
                Index::Flatten => {
                    quote! {
                        <#ty as #lib::FromSqlRow>::from_row(row)?
                    }
                }
            };

            quote! {
                let #ident: #ty = #getter;
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
    Flatten,
    Name(Ident),
}

fn attribute_items<'a>(
    name: &str,
    attrs: impl IntoIterator<Item = &'a Attribute>,
) -> Result<Vec<Meta>> {
    let mut items = Vec::new();

    for attr in attrs {
        if !attr.path.is_ident(name) {
            continue;
        }

        let meta = attr.parse_meta()?;
        let list = match meta {
            Meta::List(list) => list,
            _ => return Err(Error::new(attr.span(), "expected list: #[row(...)]")),
        };

        for inner in list.nested {
            match inner {
                NestedMeta::Lit(_) => return Err(Error::new(inner.span(), "unexpected literal")),
                NestedMeta::Meta(item) => items.push(item),
            }
        }
    }

    Ok(items)
}

fn extract_columns(input: &DeriveInput) -> Result<Vec<Column>> {
    match &input.data {
        Data::Struct(data) => {
            let items = attribute_items("row", &input.attrs)?;

            let is_ordered = has_ident("order", &items);

            let columns = data
                .fields
                .iter()
                .map(|field| -> Result<_> {
                    let items = attribute_items("row", &field.attrs)?;

                    let flatten = has_ident("flatten", &items);

                    let index = match &field.ident {
                        None => Index::Position,
                        Some(_) if is_ordered => Index::Position,
                        Some(_) if flatten => Index::Flatten,
                        Some(name) => {
                            if let Some(literal) = get_key("rename", &items) {
                                let name = lit_string(literal)?;
                                Index::Name(Ident::new(&name, literal.span()))
                            } else {
                                Index::Name(name.clone())
                            }
                        }
                    };

                    let column = Column {
                        index,
                        field: field.clone(),
                    };

                    Ok(column)
                })
                .collect::<Result<_>>()?;

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

fn meta_is_ident(ident: &str, meta: &Meta) -> bool {
    match meta {
        Meta::Path(path) if path.is_ident(ident) => true,
        _ => false,
    }
}

fn has_ident<'a>(ident: &str, items: impl IntoIterator<Item = &'a Meta>) -> bool {
    items
        .into_iter()
        .find(|item| meta_is_ident(ident, item))
        .is_some()
}

fn get_key<'a>(ident: &str, items: impl IntoIterator<Item = &'a Meta>) -> Option<&'a Lit> {
    items.into_iter().find_map(|item| match item {
        Meta::NameValue(pair) if pair.path.is_ident(ident) => Some(&pair.lit),
        _ => None,
    })
}

fn lit_string(lit: &Lit) -> Result<String> {
    match lit {
        Lit::Str(text) => Ok(text.value()),
        _ => Err(Error::new(lit.span(), "expected string literal")),
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
