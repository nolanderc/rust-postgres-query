use super::attrs::Attr;
use super::{field_initializers, Index, PartitionKind, Property};
use proc_macro2::{Span, TokenStream};
use quote::*;
use std::mem;
use syn::{Ident, Result};

struct ExactPartition {
    len: TokenStream,
    properties: Vec<Property>,
}

enum Split {
    Column(String),
    Group(Vec<Property>),
}

pub(super) fn partition_initializers(
    props: Vec<Property>,
    kind: Attr<PartitionKind>,
) -> Result<(TokenStream, Vec<Ident>)> {
    match kind.value {
        PartitionKind::Exact => {
            let partitions = exact::partition(props)?;
            Ok(exact::initializers(partitions))
        }
        PartitionKind::Split => {
            let splits = split::partition(props);

            let split_count = splits
                .iter()
                .filter(|split| match split {
                    Split::Column(_) => true,
                    _ => false,
                })
                .count();

            if split_count == 0 {
                return Err(err!(
                    kind.span,
                    "using split partitioning without any `#[row(split = \"...\")]` points"
                ));
            }

            Ok(split::initializers(splits))
        }
    }
}

mod exact {
    use super::*;

    pub(super) fn partition(props: Vec<Property>) -> Result<Vec<ExactPartition>> {
        let mut partitions = Vec::new();
        let mut props = props.into_iter().peekable();

        let merge = |prop: &Property| match prop.index {
            Index::Position | Index::Name(_) => prop.attrs.stride.is_none(),
            _ => false,
        };

        while let Some(prop) = props.next() {
            match prop {
                prop if prop.attrs.stride.is_some() => {
                    let stride = prop.attrs.stride.unwrap().value;
                    partitions.push(ExactPartition {
                        len: quote! { #stride },
                        properties: vec![prop],
                    });
                }

                prop if merge(&prop) => {
                    let mut properties = vec![prop];

                    while let Some(prop) = props.peek() {
                        if merge(prop) {
                            properties.push(props.next().unwrap());
                        } else {
                            break;
                        }
                    }

                    let len = properties.len();
                    partitions.push(ExactPartition {
                        len: quote! { #len },
                        properties,
                    });
                }

                prop if is_match!(prop.index, Index::Flatten) => {
                    let ty = &prop.ty;
                    let lib = lib!();
                    let len = quote! {
                        <#ty as #lib::FromSqlRow>::COLUMN_COUNT
                    };
                    partitions.push(ExactPartition {
                        len,
                        properties: vec![prop],
                    });
                }

                _ => return Err(err!(prop.span, "failed to compute `stride` for field")),
            }
        }

        Ok(partitions)
    }

    pub(super) fn initializers(partitions: Vec<ExactPartition>) -> (TokenStream, Vec<Ident>) {
        let mut getters = Vec::new();
        let mut locals = Vec::new();

        getters.push(quote! { let begin = 0; });

        let mut previous_end = Ident::new("begin", Span::call_site());

        for (i, partition) in partitions.into_iter().enumerate() {
            let end = Ident::new(&format!("end_{}", i), Span::call_site());
            let current = Ident::new(&format!("slice_{}", i), Span::call_site());
            let len = partition.len;

            let lib = lib!();
            let advance = quote! {
                let #end = #previous_end + #len;
                let #current = #lib::extract::Row::slice(row, #previous_end..#end)?;
                let #current = &#current;
            };

            previous_end = end;

            let (initializers, idents) = field_initializers(&partition.properties, &current);

            locals.extend(idents);

            let getter = quote! {
                #advance
                #initializers
            };

            getters.push(getter);
        }

        let getters = quote! {
            #(#getters)*
        };

        (getters, locals)
    }
}

mod split {
    use super::*;

    pub(super) fn partition(props: Vec<Property>) -> Vec<Split> {
        let mut splits = Vec::new();
        let mut group = Vec::new();

        for prop in props {
            let mut split_column = |name: String| {
                if !group.is_empty() {
                    splits.push(Split::Group(mem::take(&mut group)));
                }
                splits.push(Split::Column(name));
            };

            for name in &prop.attrs.splits {
                split_column(name.value.clone());
            }

            group.push(prop);
        }

        if !group.is_empty() {
            splits.push(Split::Group(group))
        }

        splits
    }

    pub(super) fn initializers(layout: Vec<Split>) -> (TokenStream, Vec<Ident>) {
        let mut fragments = Vec::new();
        let mut locals = Vec::new();

        let splits = layout.iter().filter_map(|kind| match kind {
            Split::Column(name) => Some(name.as_str()),
            _ => None,
        });

        let partition_ident = |i| Ident::new(&format!("partition_{}", i), Span::call_site());
        let first_partition = partition_ident(0);

        let lib = lib!();
        let row_trait = quote! { #lib::extract::Row };

        fragments.push(quote! {
            let columns = #row_trait::columns(row);
            let splits: &[&'static str] = &[#(#splits),*];
            let mut splits = #lib::extract::split_columns_many(columns, &splits);
        });

        let next_partition = quote! {
            #row_trait::slice(row, splits.next().unwrap()?)?
        };

        let advance = |partition: &Ident| {
            quote! {
                let #partition = #next_partition;
                let #partition = &#partition;
            }
        };

        fragments.push(advance(&first_partition));

        let mut splits = 0;
        let mut partition = first_partition;

        for kind in layout.iter() {
            match kind {
                Split::Column(_) => {
                    splits += 1;
                    partition = partition_ident(splits);
                    fragments.push(advance(&partition));
                }
                Split::Group(props) => {
                    let (initializers, idents) = field_initializers(&props, &partition);
                    fragments.push(initializers);
                    locals.extend(idents);
                }
            }
        }

        let getters = quote! {
            #(#fragments)*
        };

        (getters, locals)
    }
}
