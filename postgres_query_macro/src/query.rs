use proc_macro2::TokenStream;
use quote::*;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{
    Expr, ExprAssign, ExprLit, ExprPath, ExprReference, Ident, Lit, LitStr, Path, PathArguments,
    Result, Token,
};

pub struct QueryInput {
    text: Expr,
    arguments: Vec<(Ident, Expr)>,
}

impl Parse for QueryInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut parameters = Punctuated::<Expr, Token![,]>::parse_terminated(input)?.into_iter();

        let text = parameters
            .next()
            .ok_or_else(|| input.error("argument missing: expected SQL query"))?;

        let arguments = parameters.map(expr_to_argument).collect::<Result<_>>()?;

        Ok(QueryInput { text, arguments })
    }
}

impl QueryInput {
    pub fn convert_to_struct(self) -> TokenStream {
        match self.text {
            Expr::Lit(ExprLit {
                lit: Lit::Str(text),
                ..
            }) => {
                let (sql, parameters) = match parameter_substitution(text, self.arguments) {
                    Ok(result) => result,
                    Err(e) => return e.to_compile_error(),
                };

                let parameters = parameters
                    .into_iter()
                    .map(|expr| quote! { &#expr })
                    .collect::<Punctuated<_, Token![,]>>();

                quote! {
                    postgres_query::Query {
                        sql: #sql,
                        parameters: vec![#parameters],
                    }
                }
            }

            _ => err!(self.text, "expected string literal").to_compile_error(),
        }
    }
}

fn parameter_substitution(
    text: LitStr,
    arguments: Vec<(Ident, Expr)>,
) -> Result<(String, Vec<Expr>)> {
    let value = text.value();
    let mut chars = value.chars().peekable();

    let mut sql = String::with_capacity(value.len());
    let mut used = vec![false; arguments.len()];

    while let Some(ch) = chars.next() {
        if ch == '$' {
            match chars.peek() {
                Some('$') => sql.push(chars.next().unwrap()),
                _ => {
                    let mut name = String::new();

                    while let Some(&ch) = chars.peek() {
                        if ch.is_ascii_alphanumeric() || ch == '_' {
                            name.push(chars.next().unwrap());
                        } else {
                            break;
                        }
                    }

                    let index = arguments
                        .iter()
                        .position(|(ident, _)| ident == &name)
                        .ok_or_else(|| err!(text, "undefined argument `{}`", name))?;

                    used[index] = true;

                    sql.push_str(&format!("${}", index + 1));
                }
            }
        } else {
            sql.push(ch);
        }
    }

    if let Some(unused) = used.into_iter().position(|used| !used) {
        let (ident, _) = &arguments[unused];
        Err(err!(ident, "unused argument"))
    } else {
        let parameters = arguments.into_iter().map(|(_, expr)| expr).collect();
        Ok((sql, parameters))
    }
}

fn expr_to_argument(expr: Expr) -> Result<(Ident, Expr)> {
    match expr {
        Expr::Assign(assign) => {
            let ExprAssign { left, right, .. } = assign;

            let ident = expr_as_ident(&left).ok_or_else(|| err!(left, "expected an identifier"))?;

            Ok((ident.clone(), *right))
        }

        Expr::Path(_) => {
            if let Some(ident) = expr_as_ident(&expr) {
                Ok((ident.clone(), expr))
            } else {
                Err(err!(expr, "expected an identifier"))
            }
        }

        Expr::Reference(ExprReference {
            expr: ref inner, ..
        }) => {
            if let Some(ident) = expr_as_ident(&inner) {
                Ok((ident.clone(), expr))
            } else {
                Err(err!(expr, "expected an identifier"))
            }
        }

        _ => Err(err!(
            expr,
            "unexpected expression, expected either `<ident>` or `<ident> = <expr>`",
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
