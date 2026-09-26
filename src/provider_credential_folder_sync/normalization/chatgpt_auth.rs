//! Read nested login fields and JWT metadata; decoding does not authenticate claims.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde_json::Value;

pub(super) fn chatgpt_web_import_string<'a>(
    raw_map: &'a serde_json::Map<String, Value>,
    candidate_paths: &[&[&str]],
) -> Option<&'a str> {
    candidate_paths.iter().find_map(|path| {
        let mut current = raw_map.get(*path.first()?)?;
        for segment in &path[1..] {
            current = current.get(*segment)?;
        }
        current
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    })
}

pub(super) fn json_string_at_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    current
        .as_str()
        .map(str::trim)
        .filter(|candidate| !candidate.is_empty())
}

pub(super) fn decode_jwt_claims(token: &str) -> Option<Value> {
    let mut parts = token.split('.');
    let _header = parts.next()?;
    let claims = parts.next()?;
    let decoded = URL_SAFE_NO_PAD.decode(claims.as_bytes()).ok()?;
    serde_json::from_slice::<Value>(&decoded).ok()
}
