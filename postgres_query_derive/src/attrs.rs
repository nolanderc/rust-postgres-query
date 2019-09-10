use proc_macro2::Span;
use std::collections::HashMap;
use syn::*;

pub struct Attrs {
    query_literal: LitStr,
}

impl Attrs {
    pub fn from_input(input: &DeriveInput) -> Result<Attrs> {
        let mut query_literal = None;

        for attr in &input.attrs {
            if attr.path.is_ident("query") {
                let meta_list = attr
                    .parse_meta()
                    .and_then(|meta| match meta {
                        Meta::List(list) => Ok(list.nested.into_iter()),
                        _ => Err(Error::new_spanned(meta, "Expected a list")),
                    })?
                    .map(|nested| match nested {
                        NestedMeta::Meta(meta) => Ok(meta),
                        NestedMeta::Lit(lit) => Err(Error::new_spanned(
                            lit,
                            "Expected a meta item, fonud literal",
                        )),
                    })
                    .try_fold(Vec::new(), |mut acc, meta| -> Result<_> {
                        acc.push(meta?);
                        Ok(acc)
                    })?;

                for meta in meta_list {
                    match meta {
                        Meta::NameValue(ref pair) if pair.path.is_ident("sql") => {
                            let literal = match pair.lit {
                                Lit::Str(ref literal) => literal.clone(),
                                _ => {
                                    return Err(Error::new_spanned(
                                        &pair.lit,
                                        "Expected a string literal",
                                    ))
                                }
                            };

                            if query_literal.is_none() {
                                query_literal = Some(literal);
                            } else {
                                return Err(Error::new_spanned(&pair.lit, "`sql` defined twice"));
                            }
                        }

                        _ => return Err(Error::new_spanned(meta, "Unexpected meta item")),
                    }
                }
            }
        }

        let query_literal = query_literal.ok_or(Error::new(
            Span::call_site(),
            "missing #[query(sql = \"....\")]",
        ));

        Ok(Attrs {
            query_literal: query_literal?,
        })
    }

    pub fn parse_sql_literal(&self) -> Result<(String, HashMap<Ident, usize>)> {
        let text = self.query_literal.value();

        let mut chars = text.chars().peekable();

        let mut sql = String::new();
        let mut idents = HashMap::new();

        while let Some(ch) = chars.next() {
            if ch == '$' {
                if chars.peek().copied() == Some('$') {
                    sql.push('$');
                } else {
                    let mut ident = String::new();

                    while let Some(ch) = chars.peek().copied() {
                        if ch.is_alphanumeric() || ch == '_' {
                            ident.push(chars.next().unwrap())
                        } else {
                            break;
                        }
                    }

                    let ident = parse_str(&ident)?;

                    let next_index = idents.len() + 1;
                    let index = *idents.entry(ident).or_insert(next_index);

                    sql.push('$');
                    sql.push_str(&index.to_string())
                }
            } else {
                sql.push(ch);
            }
        }

        Ok((sql, idents))
    }
}
