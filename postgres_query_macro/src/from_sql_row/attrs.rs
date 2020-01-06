use proc_macro2::Span;
use std::ops::Deref;
use syn::{spanned::Spanned, Attribute, Lit, Meta, NestedMeta, Result};

pub struct ContainerAttributes {
    pub partition: Option<Attr<PartitionKind>>,
}

pub struct FieldAttributes {
    pub flatten: bool,
    pub rename: Option<String>,
    pub splits: Vec<String>,
}

pub struct Attr<T> {
    pub source: Span,
    pub value: T,
}

pub enum PartitionKind {
    Exact,
    Split(Option<String>),
}

impl<T> Attr<T> {
    pub fn new(span: impl Spanned, value: T) -> Self {
        Attr {
            source: span.span(),
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
                "malformed attribute, expected: ",
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
    (@format: $name:literal, Path) => { concat!("`", $name, "`") };
    (@format: $name:literal, NameValue) => { concat!("`", $name, " = \"...\"`") };
    (@format: $name:literal, List) => { concat!("`", $name, "(...)`") };
}

impl ContainerAttributes {
    pub fn from_attrs<'a>(
        attrs: impl IntoIterator<Item = &'a Attribute>,
    ) -> Result<ContainerAttributes> {
        let items = attribute_items("row", attrs)?;

        let mut partition = None;

        for item in items {
            use Meta::{NameValue, Path};
            match &item {
                item if item.path().is_ident("exact") => match item {
                    Path(_) => {
                        let kind = Attr::new(item, PartitionKind::Exact);
                        set_or_err!(partition, kind, err_multiple_partition!(item))?;
                    }
                    _ => return Err(err_expected_variant!(item, "exact", [Path])),
                },
                item if item.path().is_ident("split") => match item {
                    Path(_) => {
                        let kind = Attr::new(item, PartitionKind::Split(None));
                        set_or_err!(partition, kind, err_multiple_partition!(item))?;
                    }
                    NameValue(pair) => {
                        let text = lit_string(&pair.lit)?;
                        let kind = Attr::new(item, PartitionKind::Split(Some(text)));
                        set_or_err!(partition, kind, err_multiple_partition!(item))?;
                    }
                    _ => return Err(err_expected_variant!(item, "split", [Path, NameValue])),
                },
                item => return Err(err!(item, "unknown attribute",)),
            }
        }

        let container = ContainerAttributes { partition };

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

        for item in items {
            use Meta::{NameValue, Path};
            match &item {
                Path(path) if path.is_ident("flatten") => {
                    set_or_err!(flatten, true, err_duplicate_attribute!(item, "flatten"))?
                }

                NameValue(pair) if pair.path.is_ident("rename") => {
                    let text = lit_string(&pair.lit)?;
                    set_or_err!(rename, text, err_duplicate_attribute!(item, "rename"))?;
                }

                NameValue(pair) if pair.path.is_ident("split") => {
                    let text = lit_string(&pair.lit)?;
                    splits.push(text);
                }

                item => return Err(err!(item, "unknown attribute")),
            }
        }

        let field = FieldAttributes {
            flatten: flatten.unwrap_or(false),
            rename,
            splits,
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
