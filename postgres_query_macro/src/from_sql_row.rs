mod attrs;
mod partition;

use attrs::{ContainerAttributes, FieldAttributes, PartitionKind};
use partition::partition_initializers;
use proc_macro2::{Span, TokenStream};
use quote::*;
use syn::{
    token::{Enum, Union},
    Data, DataEnum, DataStruct, DataUnion, DeriveInput, Fields, Ident, Result, Type,
};

pub fn derive(input: DeriveInput) -> TokenStream {
    let ident = &input.ident;

    let Extractor {
        getters,
        locals,
        columns,
    } = match extract_columns(&input) {
        Ok(columns) => columns,
        Err(e) => return e.to_compile_error(),
    };

    let constructor = make_constructor(&input, locals);

    let lib = lib!();
    quote! {
        impl #lib::FromSqlRow for #ident {
            const COLUMN_COUNT: usize = #columns;

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

enum Index {
    Position,
    Flatten,
    Name(String),
}

struct Extractor {
    getters: TokenStream,
    locals: Vec<Ident>,
    columns: TokenStream,
}

struct Property {
    ident: Ident,
    ty: Type,
    attrs: FieldAttributes,
    index: Index,
}

fn extract_columns(input: &DeriveInput) -> Result<Extractor> {
    match &input.data {
        Data::Struct(data) => {
            let container = ContainerAttributes::from_attrs(&input.attrs)?;

            let props = extract_properties(&data)?;

            let columns = count_columns(&props);

            let (getters, locals) = if let Some(kind) = container.partition {
                partition_initializers(props, kind)
            } else {
                let row = Ident::new("row", Span::call_site());
                field_initializers(props, &row)
            };

            Ok(Extractor {
                getters: getters.into_iter().collect(),
                locals,
                columns,
            })
        }
        Data::Enum(DataEnum {
            enum_token: Enum { span },
            ..
        })
        | Data::Union(DataUnion {
            union_token: Union { span, .. },
            ..
        }) => Err(err!(
            *span,
            "`FromSqlRow` may only be derived for `struct`s"
        )),
    }
}

fn extract_properties(data: &DataStruct) -> Result<Vec<Property>> {
    let mut props = Vec::new();

    for (i, field) in data.fields.iter().enumerate() {
        let attrs = FieldAttributes::from_attrs(&field.attrs)?;

        let index = match &field.ident {
            None => Index::Position,
            Some(_) if attrs.flatten => Index::Flatten,
            Some(name) => {
                if let Some(name) = attrs.rename.clone() {
                    Index::Name(name)
                } else {
                    Index::Name(name.to_string())
                }
            }
        };

        let ident = field
            .ident
            .clone()
            .unwrap_or_else(|| Ident::new(&format!("column_{}", i), Span::call_site()));

        props.push(Property {
            ident,
            ty: field.ty.clone(),
            attrs,
            index,
        });
    }

    Ok(props)
}

fn field_initializers(props: Vec<Property>, row: &Ident) -> (Vec<TokenStream>, Vec<Ident>) {
    let mut initializers = Vec::new();
    let mut idents = Vec::new();

    for (i, prop) in props.into_iter().enumerate() {
        let ident = prop.ident;
        let ty = prop.ty;
        let lib = lib!();

        let getter = match prop.index {
            Index::Position => quote! {
                #lib::extract::Row::try_get(#row, #i)?
            },
            Index::Name(name) => quote! {
                #lib::extract::Row::try_get(#row, #name)?
            },
            Index::Flatten => quote! {
                <#ty as #lib::FromSqlRow>::from_row(#row)?
            },
        };

        let initializer = quote! {
            let #ident: #ty = #getter;
        };

        idents.push(ident);
        initializers.push(initializer);
    }

    (initializers, idents)
}

fn count_columns(props: &[Property]) -> TokenStream {
    let mut external = Vec::new();
    let mut fields: usize = 0;

    for prop in props {
        match prop.index {
            Index::Position | Index::Name(_) => fields += 1,
            Index::Flatten => {
                let ty = &prop.ty;
                let lib = lib!();
                let count = quote! { <#ty as #lib::FromSqlRow>::COLUMN_COUNT };
                external.push(count);
            }
        }
    }

    quote! { #fields #(+ #external)* }
}
