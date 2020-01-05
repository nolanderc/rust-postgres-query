use syn::{Attribute, Ident, Lit, Meta, MetaList, NestedMeta, Result};

pub struct ContainerAttributes {
    pub partition: Option<PartitionKind>,
}

pub struct FieldAttributes {
    pub flatten: bool,
    pub rename: Option<String>,
}

pub enum PartitionKind {
    Exact,
    Split(Option<String>),
}

macro_rules! set_or_err {
    ($item:ident, $var:ident, $val:expr) => {
        if $var.is_some() {
            Err(err!($item, "attribute specified twice"))
        } else {
            $var = Some($val);
            Ok(())
        }
    };
}

impl ContainerAttributes {
    pub fn from_attrs<'a>(
        attrs: impl IntoIterator<Item = &'a Attribute>,
    ) -> Result<ContainerAttributes> {
        let items = attribute_items("row", attrs)?;

        let mut partition = None;

        for item in items {
            use Meta::{NameValue, Path};
            let result = match meta_ident(&item)?.to_string().as_str() {
                "partition" => expect_list("partition", &item, |list| {
                    for arg in nested_meta(list)? {
                        match arg {
                            Path(path) if path.is_ident("exact") => {
                                set_or_err!(arg, partition, PartitionKind::Exact)?
                            }
                            Path(path) if path.is_ident("split") => {
                                set_or_err!(arg, partition, PartitionKind::Split(None))?
                            }
                            NameValue(pair) if pair.path.is_ident("split") => {
                                let text = lit_string(&pair.lit)?;
                                set_or_err!(arg, partition, PartitionKind::Split(Some(text)))?
                            }
                            _ => return Err(err!(arg, "unexpected argument")),
                        }
                    }
                    Ok(())
                }),
                ident => Err(err!(item, "unknown attribute: `{}`", ident)),
            };

            if let Err(e) = result {
                return Err(e);
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

        for item in items {
            use Meta::{NameValue, Path};
            match &item {
                Path(path) if path.is_ident("flatten") => set_or_err!(item, flatten, true)?,

                NameValue(pair) if pair.path.is_ident("rename") => {
                    let text = lit_string(&pair.lit)?;
                    set_or_err!(pair, rename, text)?;
                }

                item => return Err(err!(item, "unknown attribute")),
            }
        }

        let field = FieldAttributes {
            flatten: flatten.unwrap_or(false),
            rename,
        };

        Ok(field)
    }
}

fn expect_list(name: &str, item: &Meta, mut f: impl FnMut(&MetaList) -> Result<()>) -> Result<()> {
    match item {
        Meta::List(list) => f(list),
        Meta::Path(path) => Err(err!(path, "expected arguments: `{}(...)`", name)),
        Meta::NameValue(pair) => Err(err!(pair.lit, "unexpected value")),
    }
}

fn nested_meta(list: &MetaList) -> Result<Vec<&Meta>> {
    list.nested
        .iter()
        .map(|nested| match nested {
            NestedMeta::Meta(meta) => Ok(meta),
            NestedMeta::Lit(literal) => Err(err!(literal, "expected identifier or key-value pair")),
        })
        .collect()
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

fn meta_ident(meta: &Meta) -> Result<&Ident> {
    let path = meta.path();
    path.get_ident()
        .ok_or_else(|| err!(path, "expected an identifier, found path"))
}
