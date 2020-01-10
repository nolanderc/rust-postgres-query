use super::Parameter;
use crate::error::{Error, ParseError, Result};
use std::fmt::Write;
use std::iter::Peekable;

pub fn parse<'a>(
    text: &str,
    bindings: &[(&str, Parameter<'a>)],
) -> Result<(String, Vec<Parameter<'a>>)> {
    let mut sql = String::with_capacity(text.len());
    let mut parameters = Vec::with_capacity(bindings.len());
    let mut param_indices = vec![None; bindings.len()];

    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '$' {
            sql.push(ch);
        } else if let Some('$') = chars.peek() {
            sql.push(chars.next().unwrap());
        } else {
            let name = next_identifier(&mut chars)?;

            let argument = bindings
                .iter()
                .position(|(binding, _)| *binding == name)
                .ok_or_else(|| ParseError::UndefinedBinding { binding: name })?;

            let index = param_indices[argument].unwrap_or_else(|| {
                let (_, value) = bindings[argument];
                parameters.push(value);
                let index = parameters.len();
                param_indices[argument] = Some(index);
                index
            });

            write!(sql, "${}", index).unwrap();
        }
    }

    Ok((sql, parameters))
}

fn next_identifier(chars: &mut Peekable<impl Iterator<Item = char>>) -> Result<String> {
    let mut name = String::new();

    while let Some(&ch) = chars.peek() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            name.push(chars.next().unwrap());
        } else {
            break;
        }
    }

    if name.is_empty() {
        let found = chars.peek().copied();
        return Err(Error::from(ParseError::EmptyIdentifier { found }));
    }

    Ok(name)
}
