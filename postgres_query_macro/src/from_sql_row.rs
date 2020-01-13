mod attrs;
mod partition;
mod validate;

use attrs::{ContainerAttributes, FieldAttributes, MergeKind, PartitionKind};
use partition::partition_initializers;
use proc_macro2::{Span, TokenStream};
use quote::*;
use syn::{
    spanned::Spanned,
    token::{Enum, Union},
    Data, DataEnum, DataStruct, DataUnion, DeriveInput, Field, Fields, Ident, Result, Type,
};
use validate::validate_properties;

pub fn derive(input: DeriveInput) -> TokenStream {
    let ident = &input.ident;

    let Extractor {
        getters,
        locals,
        columns,
        merge,
    } = match extract_columns(&input) {
        Ok(columns) => columns,
        Err(e) => return e.to_compile_error(),
    };

    let constructor = make_constructor(&input, locals);

    let multi = merge.map(|merge| make_merge(merge, &constructor, &getters));

    let lib = lib!();
    quote! {
        impl #lib::FromSqlRow for #ident {
            const COLUMN_COUNT: usize = #columns;

            fn from_row<R>(__row: &R) -> Result<Self, #lib::extract::Error>
            where
                R: #lib::extract::Row
            {
                #getters
                Ok(#constructor)
            }

            #multi
        }
    }
}

fn make_constructor(input: &DeriveInput, locals: impl IntoIterator<Item = Local>) -> TokenStream {
    let ident = &input.ident;

    let mut locals = locals.into_iter().map(|local| {
        let ident = local.ident;
        let lib = lib!();
        match local.merge {
            None => (ident.clone(), quote! { #ident }),
            Some(base) => (
                ident.clone(),
                quote! {
                    {
                        let mut collections = <#base as Default>::default();
                        #lib::extract::Merge::insert(&mut collections, #ident);
                        collections
                    }
                },
            ),
        }
    });

    match &input.data {
        Data::Struct(data) => match data.fields {
            Fields::Unnamed(_) => {
                let values = locals.map(|(_, value)| value);
                quote! {
                    #ident ( #(#values),* )
                }
            }
            Fields::Named(_) => {
                let fields = locals.map(|(ident, value)| quote! { #ident: #value });
                quote! {
                    #ident { #(#fields),* }
                }
            }
            Fields::Unit => {
                if locals.next().is_none() {
                    quote! {
                        #ident
                    }
                } else {
                    unreachable!("Attempted to construct unit struct with fields");
                }
            }
        },
        _ => unreachable!(),
    }
}

fn make_merge(merge: Merge, constructor: &TokenStream, getters: &TokenStream) -> TokenStream {
    let lib = lib!();

    let Merge {
        kind,
        keys,
        collections,
    } = merge;

    let key_idents = keys.iter().map(|(ident, _)| ident).collect::<Vec<_>>();
    let collection_idents = collections
        .iter()
        .map(|(ident, _)| ident)
        .collect::<Vec<_>>();

    let body = match kind {
        MergeKind::Group => {
            quote! {
                let mut __objects = Vec::<Self>::new();
                for __row in __rows {
                    #getters

                    if let Some(__last) = __objects.last_mut() {
                        if #(#key_idents == __last.#key_idents) && * {
                            #(
                                #lib::extract::Merge::insert(
                                    &mut __last.#collection_idents,
                                    #collection_idents
                                );
                            )*
                        } else {
                            __objects.push(#constructor);
                        }
                    } else {
                        __objects.push(#constructor);
                    }
                }
                Ok(__objects)
            }
        }

        MergeKind::Hash => {
            let key_types = keys.iter().map(|(_, ty)| ty);

            quote! {
                let mut __objects = Vec::<Self>::new();
                let mut __indices = ::std::collections::HashMap::<(#(#key_types,)*), usize>::new();

                for __row in __rows {
                    #getters

                    let __key = (#(#key_idents,)*);

                    if let Some(&__index) = __indices.get(&__key) {
                        #(
                            #lib::extract::Merge::insert(
                                &mut __objects[__index].#collection_idents,
                                #collection_idents
                            );
                        )*
                    } else {
                        let __index = __objects.len();
                        __indices.insert(__key.clone(), __index);
                        let (#(#key_idents,)*) = __key;
                        __objects.push(#constructor);
                    }
                }

                Ok(__objects)
            }
        }
    };

    quote! {
        fn from_row_multi<R>(__rows: &[R]) -> Result<Vec<Self>, #lib::extract::Error>
        where
            R: #lib::extract::Row
        {
            #body
        }
    }
}

enum Index {
    Position,
    Flatten,
    Name(String),
}

struct Extractor {
    getters: TokenStream,
    locals: Vec<Local>,
    columns: TokenStream,
    merge: Option<Merge>,
}

struct Local {
    ident: Ident,
    merge: Option<Type>,
}

struct Merge {
    kind: MergeKind,
    keys: Vec<(Ident, Type)>,
    collections: Vec<(Ident, Type)>,
}

struct Property {
    ident: Ident,
    ty: Type,
    attrs: FieldAttributes,
    index: Index,
    span: Span,
    field: Field,
}

fn extract_columns(input: &DeriveInput) -> Result<Extractor> {
    match &input.data {
        Data::Struct(data) => {
            let container = ContainerAttributes::from_attrs(&input.attrs)?;
            let props = extract_properties(&data)?;

            validate_properties(&container, &props)?;

            let columns = count_columns(&props);

            let merge = extract_merge(&container, &props);

            let (getters, locals) = if let Some(kind) = container.partition {
                partition_initializers(props, kind)?
            } else {
                let row = Ident::new("__row", Span::call_site());
                field_initializers(&props, &row)
            };

            Ok(Extractor {
                getters,
                locals,
                columns,
                merge,
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

fn extract_merge(container: &ContainerAttributes, props: &[Property]) -> Option<Merge> {
    container.merge.map(|kind| Merge {
        kind: kind.value,
        keys: props
            .iter()
            .filter_map(|prop| match prop.attrs.key {
                Some(_) => Some((prop.ident.clone(), prop.ty.clone())),
                None => None,
            })
            .collect(),
        collections: props
            .iter()
            .filter_map(|prop| match prop.attrs.merge {
                Some(_) => Some((prop.ident.clone(), prop.ty.clone())),
                None => None,
            })
            .collect(),
    })
}

fn extract_properties(data: &DataStruct) -> Result<Vec<Property>> {
    let mut props = Vec::new();

    for (i, field) in data.fields.iter().enumerate() {
        let attrs = FieldAttributes::from_attrs(&field.attrs)?;

        let index = match &field.ident {
            _ if attrs.merge.is_some() => Index::Flatten,
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

        let ty = if attrs.merge.is_some() {
            let base = &field.ty;
            let lib = lib!();
            let qualifier = quote! {
                <#base as #lib::extract::Merge>::Item
            };
            syn::parse2(qualifier)?
        } else {
            field.ty.clone()
        };

        props.push(Property {
            ident,
            ty,
            attrs,
            index,
            span: field.span(),
            field: field.clone(),
        });
    }

    Ok(props)
}

fn field_initializers(props: &[Property], row: &Ident) -> (TokenStream, Vec<Local>) {
    let mut initializers = Vec::new();
    let mut locals = Vec::new();

    for (i, prop) in props.iter().enumerate() {
        let ident = &prop.ident;
        let ty = &prop.ty;
        let lib = lib!();

        let getter = match &prop.index {
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

        initializers.push(quote! {
            let #ident: #ty = #getter;
        });

        let merge = prop.attrs.merge.map(|_| prop.field.ty.clone());
        locals.push(Local {
            ident: ident.clone(),
            merge,
        });
    }

    let initializers = quote! {
        #(#initializers)*
    };

    (initializers, locals)
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

    quote! {
        #fields #(+ #external)*
    }
}
