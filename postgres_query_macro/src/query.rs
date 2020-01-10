use proc_macro2::TokenStream;
use quote::*;
use std::fmt::Write;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{
    Expr, ExprAssign, ExprLit, ExprPath, ExprRange, ExprReference, Ident, Lit, LitStr, Path,
    PathArguments, RangeLimits, Result, Token,
};

pub struct QueryInput {
    text: Expr,
    arguments: Vec<Argument>,
}

enum Argument {
    Single { ident: Ident, value: Expr },
    Dynamic { value: Expr },
}

impl Parse for QueryInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut parameters = Punctuated::<Expr, Token![,]>::parse_terminated(input)?.into_iter();

        let text = parameters
            .next()
            .ok_or_else(|| input.error("argument missing: expected SQL query"))?;

        let arguments: Vec<_> = parameters.map(expr_to_argument).collect::<Result<_>>()?;

        Ok(QueryInput { text, arguments })
    }
}

impl QueryInput {
    pub fn convert_to_struct_static(self) -> Result<TokenStream> {
        match self.text {
            Expr::Lit(ExprLit {
                lit: Lit::Str(text),
                ..
            }) => {
                let arguments = self
                    .arguments
                    .into_iter()
                    .map(|argument| match argument {
                        Argument::Single { ident, value } => Ok((ident, value)),
                        Argument::Dynamic { value } => Err(err!(
                            value,
                            "found dynamic binding (`..<expr>`) in static context, \
                             use `query_dyn!` if working with dynamic parameters"
                        )),
                    })
                    .collect::<Result<Vec<_>>>()?;

                let (sql, parameters) = parameter_substitution(text, arguments)?;

                let lib = lib!();
                Ok(quote! {
                    #lib::Query::new_static(#sql, vec![#(&#parameters),*])
                })
            }

            _ => Err(err!(
                self.text,
                "expected a string literal, \
                 use `query_dyn!` if working with dynamically generated strings"
            )),
        }
    }

    pub fn convert_to_struct_dynamic(self) -> Result<TokenStream> {
        let mut simple = Vec::new();
        let mut dynamic = Vec::new();

        for argument in self.arguments {
            match argument {
                Argument::Single { ident, value } => {
                    let name = ident.to_string();
                    simple.push(quote! {
                        (#name, &#value)
                    });
                }
                Argument::Dynamic { value } => {
                    dynamic.push(value);
                }
            }
        }

        let text = self.text;

        let lib = lib!();
        let result = if dynamic.is_empty() {
            quote! {
                #lib::Query::parse(#text, &[#(#simple),*])
            }
        } else {
            quote! {
                {
                    let mut parameters = Vec::<(&str, #lib::Parameter)>::with_capacity(16);
                    parameters.extend_from_slice(&[#(#simple),*]);

                    #(
                        parameters.extend(#dynamic);
                    )*

                    #lib::Query::parse(#text, &parameters)
                }
            }
        };

        Ok(result)
    }
}

fn parameter_substitution(
    literal: LitStr,
    bindings: Vec<(Ident, Expr)>,
) -> Result<(String, Vec<Expr>)> {
    let text = literal.value();

    let mut sql = String::with_capacity(text.len());
    let mut parameters = Vec::with_capacity(bindings.len());
    let mut param_indices = vec![None; bindings.len()];

    let mut chars = text.chars().enumerate().peekable();

    let context = |i: usize| {
        let start = i.saturating_sub(16);
        text.chars().skip(start).take(32).collect::<String>()
    };

    while let Some((index, ch)) = chars.next() {
        if ch != '$' {
            sql.push(ch);
        } else if let Some((_, '$')) = chars.peek() {
            let (_, dollar) = chars.next().unwrap();
            sql.push(dollar);
        } else {
            let mut name = String::new();

            while let Some(&(_, ch)) = chars.peek() {
                if ch.is_ascii_alphanumeric() || ch == '_' {
                    let (_, ch) = chars.next().unwrap();
                    name.push(ch);
                } else {
                    break;
                }
            }

            if name.is_empty() {
                return Err(err!(
                    literal,
                    "expected an identifier, found {:?}. Dollar signs may be escaped: `$$`. \
                     Here: `{}`",
                    chars.peek().map(|(_, ch)| *ch).unwrap_or('\0'),
                    context(index),
                ));
            }

            let argument = bindings
                .iter()
                .position(|(binding, _)| *binding == name)
                .ok_or_else(|| {
                    err!(
                        literal,
                        "could not find a binding with the name `{}`. Here: `{}`",
                        name,
                        context(index),
                    )
                })?;

            let index = param_indices[argument].unwrap_or_else(|| {
                let (_, value) = &bindings[argument];
                parameters.push(value.clone());
                let index = parameters.len();
                param_indices[argument] = Some(index);
                index
            });

            write!(sql, "${}", index).unwrap();
        }
    }

    if let Some(index) = param_indices
        .into_iter()
        .position(|index: Option<usize>| index.is_none())
    {
        let (ident, _) = &bindings[index];
        Err(err!(ident, "unused argument"))
    } else {
        Ok((sql, parameters))
    }
}

fn expr_to_argument(expr: Expr) -> Result<Argument> {
    match expr {
        Expr::Assign(assign) => {
            let ExprAssign { left, right, .. } = assign;

            let ident = expr_as_ident(&left).ok_or_else(|| err!(left, "expected an identifier"))?;

            Ok(Argument::Single {
                ident: ident.clone(),
                value: *right,
            })
        }

        Expr::Path(_) => {
            if let Some(ident) = expr_as_ident(&expr) {
                Ok(Argument::Single {
                    ident: ident.clone(),
                    value: expr,
                })
            } else {
                Err(err!(expr, "expected an identifier"))
            }
        }

        Expr::Reference(ExprReference {
            expr: ref inner, ..
        }) => {
            if let Some(ident) = expr_as_ident(&inner) {
                Ok(Argument::Single {
                    ident: ident.clone(),
                    value: expr,
                })
            } else {
                Err(err!(expr, "expected an identifier"))
            }
        }

        Expr::Range(ExprRange {
            from: None,
            limits: RangeLimits::HalfOpen(_),
            to: Some(expr),
            ..
        }) => Ok(Argument::Dynamic { value: *expr }),

        _ => Err(err!(
            expr,
            "unexpected expression, expected either `<ident>`, `<ident> = <expr>` or `..<expr>`",
        )),
    }
}

fn path_is_ident(path: &Path) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == 1
        && match path.segments[0].arguments {
            PathArguments::None => true,
            _ => false,
        }
}

fn expr_as_ident(expr: &Expr) -> Option<&Ident> {
    match expr {
        Expr::Path(ExprPath {
            qself: None, path, ..
        }) if path_is_ident(&path) => Some(&path.segments[0].ident),
        _ => None,
    }
}
