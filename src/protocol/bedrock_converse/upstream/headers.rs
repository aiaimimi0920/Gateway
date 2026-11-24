//! Parse every typed header safely; only trusted, known discriminators reach diagnostics.
use std::collections::HashMap;

use super::{invalid, upstream_failure};
use crate::protocol::stream_error::ProtocolStreamError;
use serde_json::Value;

pub(super) struct Event {
    pub kind: String,
    pub body: Value,
}

pub(super) fn parse(
    mut bytes: &[u8],
    payload: &[u8],
    strict_json: bool,
) -> Result<Event, ProtocolStreamError> {
    let mut headers = HashMap::new();
    while !bytes.is_empty() {
        // Bound map overhead independently of wire bytes; not the service's encoded-header-size rule.
        if headers.len() >= 4096 {
            return Err(invalid("Gateway header count budget exceeded"));
        }
        let length = take(&mut bytes, 1)?[0] as usize;
        if length == 0 {
            return Err(invalid("empty header name"));
        }
        let name = text(take(&mut bytes, length)?)?;
        let kind = take(&mut bytes, 1)?[0];
        let value = match kind {
            0 | 1 => None,
            2 => {
                take(&mut bytes, 1)?;
                None
            }
            3 => {
                take(&mut bytes, 2)?;
                None
            }
            4 => {
                take(&mut bytes, 4)?;
                None
            }
            5 | 8 => {
                take(&mut bytes, 8)?;
                None
            }
            9 => {
                take(&mut bytes, 16)?;
                None
            }
            6 | 7 => {
                let length = u16::from_be_bytes(take(&mut bytes, 2)?.try_into().unwrap()) as usize;
                let value = take(&mut bytes, length)?;
                if kind == 7 {
                    Some(text(value)?)
                } else {
                    None
                }
            }
            _ => return Err(invalid("unknown header value type")),
        };
        if headers.insert(name, value).is_some() {
            return Err(invalid("duplicate header"));
        }
    }
    for name in [
        ":message-type",
        ":event-type",
        ":exception-type",
        ":content-type",
        ":error-code",
        ":error-message",
    ] {
        if headers.get(name).is_some_and(Option::is_none) {
            return Err(invalid("discriminator header is not a string"));
        }
    }
    let get = |name| headers.get(name).copied().flatten();
    if get(":content-type").is_some_and(|kind| !kind.eq_ignore_ascii_case("application/json")) {
        return Err(invalid("unsupported event content type"));
    }
    match get(":message-type") {
        Some("exception") => {
            if [":event-type", ":error-code", ":error-message"]
                .iter()
                .any(|key| get(key).is_some())
            {
                return Err(invalid("conflicting exception discriminators"));
            }
            let kind = get(":exception-type").ok_or_else(|| invalid("missing exception type"))?;
            super::super::exception::validate_payload(kind, payload)?;
            Err(upstream_failure(kind))
        }
        Some("error") => {
            if get(":event-type").is_some() || get(":exception-type").is_some() {
                return Err(invalid("conflicting error discriminators"));
            }
            get(":error-code").ok_or_else(|| invalid("missing error code"))?;
            get(":error-message").ok_or_else(|| invalid("missing error message"))?;
            Err(upstream_failure("unmodeled"))
        }
        Some("event") => {
            let kind = get(":event-type").ok_or_else(|| invalid("missing event type"))?;
            if [":exception-type", ":error-code", ":error-message"]
                .iter()
                .any(|key| get(key).is_some())
            {
                return Err(invalid("conflicting event discriminators"));
            }
            let body = if strict_json {
                crate::protocol::unique_json::parse(payload)
            } else {
                serde_json::from_slice::<Value>(payload)
            }
            .map_err(|_| invalid("invalid event JSON"))?;
            if !body.is_object() {
                return Err(invalid("event payload is not an object"));
            }
            Ok(Event {
                kind: kind.into(),
                body,
            })
        }
        _ => Err(invalid("missing or invalid message type")),
    }
}

fn take<'a>(bytes: &mut &'a [u8], count: usize) -> Result<&'a [u8], ProtocolStreamError> {
    if count > bytes.len() {
        return Err(invalid("truncated event header"));
    }
    let (head, tail) = bytes.split_at(count);
    *bytes = tail;
    Ok(head)
}

fn text(bytes: &[u8]) -> Result<&str, ProtocolStreamError> {
    std::str::from_utf8(bytes).map_err(|_| invalid("invalid header UTF-8"))
}
