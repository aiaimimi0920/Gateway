//! Constructive RFC 9535 single-location subset; never evaluate queries or allocate sparse arrays.
use super::{Contract, GatewayError, Value};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) enum Segment {
    Member(String),
    Index(usize),
}

fn space(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\r' | '\n')
}
fn name_first(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_' || ch as u32 >= 0x80
}

pub(super) fn parse(contract: Contract, input: &str) -> Result<Vec<Segment>, GatewayError> {
    let invalid = |reason| contract.invalid(reason);
    let mut rest = input
        .strip_prefix('$')
        .ok_or_else(|| invalid("Invalid argument path."))?;
    let mut path = Vec::new();
    while !rest.is_empty() {
        // 64 containers keep construction, serialization and Drop below the JSON recursion limit.
        if path.len() == 64 {
            return Err(invalid("Argument path is too deep."));
        }
        rest = rest.trim_start_matches(space);
        if let Some(tail) = rest.strip_prefix('.') {
            if !tail.chars().next().is_some_and(name_first) {
                return Err(invalid("Invalid argument member path."));
            }
            let end = tail
                .char_indices()
                .find(|(_, c)| !name_first(*c) && !c.is_ascii_digit())
                .map_or(tail.len(), |(i, _)| i);
            path.push(Segment::Member(tail[..end].to_owned()));
            rest = &tail[end..];
        } else if let Some(tail) = rest.strip_prefix('[') {
            rest = tail.trim_start_matches(space);
            let segment = if rest.starts_with(['\'', '"']) {
                let (name, tail) = quoted(contract, rest)?;
                rest = tail;
                Segment::Member(name)
            } else {
                let end = rest.bytes().take_while(u8::is_ascii_digit).count();
                let number = &rest[..end];
                if number.is_empty() || (number.len() > 1 && number.starts_with('0')) {
                    return Err(invalid("Unsupported argument index."));
                }
                let index = number
                    .parse::<usize>()
                    .map_err(|_| invalid("Argument index is too large."))?;
                rest = &rest[end..];
                Segment::Index(index)
            };
            rest = rest
                .trim_start_matches(space)
                .strip_prefix(']')
                .ok_or_else(|| invalid("Invalid argument selector."))?;
            path.push(segment);
        } else {
            return Err(invalid("Unsupported argument path."));
        }
    }
    if !matches!(path.first(), Some(Segment::Member(_))) {
        return Err(invalid("Arguments must be an object."));
    }
    Ok(path)
}

fn quoted(contract: Contract, input: &str) -> Result<(String, &str), GatewayError> {
    let invalid = |reason| contract.invalid(reason);
    let quote = input.chars().next().unwrap();
    let mut rest = &input[1..];
    let mut encoded = String::from("\"");
    while let Some(ch) = rest.chars().next() {
        rest = &rest[ch.len_utf8()..];
        match ch {
            ch if ch == quote => {
                encoded.push('"');
                return Ok((
                    serde_json::from_str(&encoded)
                        .map_err(|_| invalid("Invalid argument path escape."))?,
                    rest,
                ));
            }
            '\\' => {
                let escaped = rest
                    .chars()
                    .next()
                    .ok_or_else(|| invalid("Incomplete argument path escape."))?;
                rest = &rest[escaped.len_utf8()..];
                if escaped == quote && quote == '\'' {
                    encoded.push('\'');
                } else if escaped == quote
                    || matches!(escaped, 'b' | 'f' | 'n' | 'r' | 't' | '/' | '\\' | 'u')
                {
                    encoded.push('\\');
                    encoded.push(escaped);
                } else {
                    return Err(invalid("Invalid argument path escape."));
                }
            }
            '"' => encoded.push_str("\\\""),
            ch if ch <= '\u{1f}' => return Err(invalid("Invalid argument path character.")),
            _ => encoded.push(ch),
        }
    }
    Err(invalid("Incomplete argument member selector."))
}

pub(super) fn slot<'a>(
    contract: Contract,
    root: &'a mut Value,
    path: &[Segment],
) -> Result<(&'a mut Value, bool), GatewayError> {
    let invalid = |reason| contract.invalid(reason);
    let mut current = root;
    for (i, segment) in path.iter().enumerate() {
        let last = i + 1 == path.len();
        let container = || {
            if last {
                Value::Null
            } else if matches!(path[i + 1], Segment::Index(_)) {
                Value::Array(Vec::new())
            } else {
                Value::Object(Default::default())
            }
        };
        let (next, fresh) = match (segment, current) {
            (Segment::Member(name), Value::Object(map)) => match map.entry(name.clone()) {
                serde_json::map::Entry::Vacant(entry) => (entry.insert(container()), true),
                serde_json::map::Entry::Occupied(entry) => (entry.into_mut(), false),
            },
            (Segment::Index(index), Value::Array(items)) if *index <= items.len() => {
                let fresh = *index == items.len();
                if fresh {
                    items.push(container());
                }
                (&mut items[*index], fresh)
            }
            _ => {
                return Err(invalid(
                    "Argument path conflicts or creates a sparse array.",
                ))
            }
        };
        if last {
            return Ok((next, fresh));
        }
        current = next;
    }
    unreachable!("parse requires a nonempty member path")
}
