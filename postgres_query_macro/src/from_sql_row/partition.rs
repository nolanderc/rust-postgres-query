use super::{field_initializers, Index, PartitionKind, Property};
use proc_macro2::{Span, TokenStream};
use quote::*;
use syn::Ident;

struct ExactPartition {
    len: TokenStream,
    properties: Vec<Property>,
}

struct SplitPartition {
    split: Option<String>,
    properties: Vec<Property>,
}

pub(super) fn partition_initializers(
    props: Vec<Property>,
    kind: PartitionKind,
) -> (Vec<TokenStream>, Vec<Ident>) {
    match kind {
        PartitionKind::Exact => {
            let partitions = exact::partition(props);
            exact::initializers(partitions)
        }
        PartitionKind::Split(name) => {
            let partitions = split::partition(props, name);
            split::initializers(partitions)
        }
    }
}

mod exact {
    use super::*;

    pub(super) fn partition(props: Vec<Property>) -> Vec<ExactPartition> {
        let mut partitions = Vec::new();
        let mut props = props.into_iter().peekable();

        let standalone = |prop: &Property| match prop.index {
            Index::Position | Index::Name(_) => true,
            Index::Flatten => false,
        };

        while let Some(prop) = props.next() {
            if standalone(&prop) {
                let mut properties = Vec::new();

                while let Some(prop) = props.peek() {
                    if standalone(prop) {
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
            } else {
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
        }

        partitions
    }

    pub(super) fn initializers(partitions: Vec<ExactPartition>) -> (Vec<TokenStream>, Vec<Ident>) {
        let mut getters = Vec::new();
        let mut locals = Vec::new();

        getters.push(quote! { let mut offset = 0; });

        for (i, partition) in partitions.into_iter().enumerate() {
            let current = Ident::new(&format!("row_{}", i), Span::call_site());
            let len = partition.len;

            let lib = lib!();
            let advance = quote! {
                let ref #current = #lib::extract::Row::slice(row, offset..offset+#len)?;
                offset += #len;
            };

            let (initializer, idents) = field_initializers(partition.properties, &current);

            locals.extend(idents);

            let getter = quote! {
                #advance
                #(#initializer)*
            };

            getters.push(getter);
        }

        (getters, locals)
    }
}

mod split {
    use super::*;

    pub(super) fn partition(props: Vec<Property>, name: Option<String>) -> Vec<SplitPartition> {
        let mut partitions = Vec::new();
        let mut props = props.into_iter().peekable();

        let standalone = |prop: &Property| match prop.index {
            Index::Position | Index::Name(_) => true,
            Index::Flatten => false,
        };

        while let Some(prop) = props.next() {
            if standalone(&prop) {
                let mut properties = Vec::new();

                while let Some(prop) = props.peek() {
                    if standalone(prop) {
                        properties.push(props.next().unwrap());
                    } else {
                        break;
                    }
                }

                partitions.push(SplitPartition {
                    split: None,
                    properties,
                });
            } else {
                partitions.push(SplitPartition {
                    split: name.clone(),
                    properties: vec![prop],
                });
            }
        }

        partitions
    }

    pub(super) fn initializers(partitions: Vec<SplitPartition>) -> (Vec<TokenStream>, Vec<Ident>) {
        let mut getters = Vec::new();
        let mut locals = Vec::new();

        getters.push(quote! {
            let columns = row.columns();
            let mut begin = 0;
        });

        for (i, partition) in partitions.into_iter().enumerate() {
            let current = Ident::new(&format!("row_{}", i), Span::call_site());

            let name = partition.split;

            let lib = lib!();
            let row_trait = quote! { #lib::extract::Row };

            let advance = quote! {
                let cols = &columns[begin..columns.len()];
                let mut breaks = cols
                    .iter()
                    .enumerate()
                    .filter(|(_, col)| col.name() == #name)
                    .map(|(i, _)| begin + i);
                let start = breaks.next().unwrap();
                let end = breaks.next().unwrap_or_else(|| columns.len());
                let ref #current = #row_trait::slice(row, start..end)?;
                begin = end;
            };

            let (initializer, idents) = field_initializers(partition.properties, &current);

            locals.extend(idents);

            let getter = quote! {
                #advance
                #(#initializer)*
            };

            getters.push(getter);
        }

        (getters, locals)
    }
}
