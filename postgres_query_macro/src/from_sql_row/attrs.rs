use proc_macro2::Span;
use std::fmt::Display;
use std::ops::Deref;
use std::str::FromStr;
use syn::{spanned::Spanned, Attribute, Lit, Meta, NestedMeta, Result};

pub struct ContainerAttributes {
    pub partition: Option<Attr<PartitionKind>>,
    pub merge: Option<Attr<MergeKind>>,
}

pub struct FieldAttributes {
    pub flatten: bool,
    pub rename: Option<String>,
    pub splits: Vec<Attr<String>>,
    pub stride: Option<Attr<usize>>,
    pub key: Option<Attr<()>>,
    pub merge: Option<Attr<()>>,
}

#[derive(Copy, Clone)]
pub struct Attr<T> {
    pub span: Span,
    pub value: T,
}

#[derive(Copy, Clone)]
pub enum PartitionKind {
    Exact,
    Split,
}

#[derive(Copy, Clone)]
pub enum MergeKind {
    Group,
    Hash,
}

impl<T> Attr<T> {
    pub fn new(span: impl Spanned, value: T) -> Self {
        Attr {
            span: span.span(),
            value,
        }
    }
}

impl<T> Deref for Attr<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

macro_rules! set_or_err {
    ($var:ident, $val:expr, $err:expr) => {
        if $var.is_some() {
            Err($err)
        } else {
            $var = Some($val);
            Ok(())
        }
    };
}

macro_rules! err_duplicate_attribute {
    ($item:expr, $attr:literal) => {
        err!($item, concat!("attribute specified twice: `", $attr, "`"))
    };
}

macro_rules! err_multiple_partition {
    ($item:expr) => {
        err!(
            $item,
            "only one partitioning scheme may be specified (either `split` or `exact`)"
        )
    };
}

macro_rules! err_expected_variant {
    ($item:expr, $name:literal, [$($kind:ident),+]) => {
        err!(
            $item,
            concat!(
                "malformed attribute, expected ",
                err_expected_variant!(@concat: $name, [$($kind),+])
            )
        )
    };
    (@concat: $name:literal, [$head:ident, $mid:ident, $($tail:ident),+]) => {
        concat!(
            err_expected_variant!(@format: $name, $head), ", ",
            err_expected_variant!(@concat: $name, [$mid, $($tail),+])
        )
    };
    (@concat: $name:literal, [$head:ident, $last:ident]) => {
        concat!(
            err_expected_variant!(@format: $name, $head), " or ",
            err_expected_variant!(@format: $name, $last)
        )
    };
    (@concat: $name:literal, [$head:ident]) => {
        err_expected_variant!(@format: $name, $head)
    };
    (@format: $name:literal, Path) => { concat!("an identifier (`", $name, "`)") };
    (@format: $name:literal, NameValue) => { concat!("key-value (`", $name, " = \"...\"`)") };
    (@format: $name:literal, List) => { concat!("a list (`", $name, "(...)`)") };
}

macro_rules! match_item {
    (
        ($item:expr) {
            $(
                $ident:literal => {
                    $(
                        $meta:ident ($binding:pat) => $expr:expr
                    ),+ $(,)?
                }
            ),* $(,)?
        }
    ) => {
        match $item {
            $(
                item if item.path().is_ident($ident) => match item {
                    $(
                        $meta ($binding) => $expr,
                    )+
                    _ => return Err(err_expected_variant!(
                        item,
                        $ident,
                        [$($meta),+]
                    )),
                },
            )*
            item => return Err(err!(item, "unknown attribute")),
        }
    };
}

impl ContainerAttributes {
    pub fn from_attrs<'a>(
        attrs: impl IntoIterator<Item = &'a Attribute>,
    ) -> Result<ContainerAttributes> {
        let items = attribute_items("row", attrs)?;

        let mut partition = None;
        let mut merge = None;

        for item in &items {
            use Meta::Path;

            match_item!((item) {
                "exact" => {
                    Path(_) => {
                        let kind = Attr::new(item, PartitionKind::Exact);
                        set_or_err!(partition, kind, err_multiple_partition!(item))?;
                    }
                },
                "split" => {
                    Path(_) => {
                        let kind = Attr::new(item, PartitionKind::Split);
                        set_or_err!(partition, kind, err_multiple_partition!(item))?;
                    }
                },
                "group" => {
                    Path(_) => {
                        let kind = Attr::new(item, MergeKind::Group);
                        set_or_err!(merge, kind, err_multiple_partition!(item))?;
                    }
                },
                "hash" => {
                    Path(_) => {
                        let kind = Attr::new(item, MergeKind::Hash);
                        set_or_err!(merge, kind, err_multiple_partition!(item))?;
                    }
                },
            })
        }

        let container = ContainerAttributes { partition, merge };

        Ok(container)
    }
}

impl FieldAttributes {
    pub fn from_attrs<'a>(
        attrs: impl IntoIterator<Item = &'a Attribute>,
    ) -> Result<FieldAttributes> {
        let items = attribute_items("row", attrs)?;

        let mut flatten = None;
        let mut rename = None;
        let mut splits = Vec::new();
        let mut stride = None;
        let mut key = None;
        let mut merge = None;

        for item in &items {
            use Meta::{NameValue, Path};

            match_item!((item) {
                "flatten" => {
                    Path(_) => {
                        set_or_err!(flatten, true, err_duplicate_attribute!(item, "flatten"))?
                    }
                },
                "rename" => {
                    NameValue(pair) => {
                        let text = lit_string(&pair.lit)?;
                        set_or_err!(rename, text, err_duplicate_attribute!(item, "rename"))?;
                    }
                },
                "split" => {
                    NameValue(pair) => {
                        let text = lit_string(&pair.lit)?;
                        splits.push(Attr::new(pair, text));
                    }
                },
                "stride" => {
                    NameValue(pair) => {
                        let step = lit_int(&pair.lit)?;
                        let step = Attr::new(pair, step);
                        set_or_err!(stride, step, err_duplicate_attribute!(item, "stride"))?
                    }
                },
                "key" => {
                    Path(_) => {
                        let attr = Attr::new(item, ());
                        set_or_err!(key, attr, err_duplicate_attribute!(item, "key"))?
                    }
                },
                "merge" => {
                    Path(_) => {
                        let attr = Attr::new(item, ());
                        set_or_err!(merge, attr, err_duplicate_attribute!(item, "merge"))?
                    }
                },
            })
        }

        let field = FieldAttributes {
            flatten: flatten.unwrap_or(false),
            rename,
            splits,
            stride,
            key,
            merge,
        };

        Ok(field)
    }
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
            _ => return Err(err!(attr, "expected list: #[row(...)]")),
        };

        for inner in list.nested {
            match inner {
                NestedMeta::Lit(_) => return Err(err!(inner, "unexpected literal")),
                NestedMeta::Meta(item) => items.push(item),
            }
        }
    }

    Ok(items)
}

fn lit_string(lit: &Lit) -> Result<String> {
    match lit {
        Lit::Str(text) => Ok(text.value()),
        _ => Err(err!(lit, "expected string literal")),
    }
}

fn lit_int<N>(lit: &Lit) -> Result<N>
where
    N: FromStr,
    N::Err: Display,
{
    match lit {
        Lit::Int(int) => int.base10_parse(),
        _ => Err(err!(lit, "expected integer literal")),
    }
}
