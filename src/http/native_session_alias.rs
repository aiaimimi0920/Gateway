//! Canonical session header admission, with explicit duplicate/conflict rejection.
use crate::error::GatewayError;
use axum::http::HeaderMap;

pub(crate) fn normalize_headers(headers: &mut HeaderMap) -> Result<(), GatewayError> {
    for name in ["session-id", "session_id"] {
        if headers.get_all(name).iter().count() > 1 {
            return Err(GatewayError::bad_request(
                "Duplicate session identity header",
            ));
        }
        if let Some(value) = headers.get(name) {
            if !value
                .to_str()
                .ok()
                .is_some_and(|value| crate::protocol::codex_wire::valid_header("session-id", value))
            {
                return Err(GatewayError::bad_request("Invalid session identity header"));
            }
        }
    }
    if let Some(alias) = headers.remove("session_id") {
        if let Some(canonical) = headers.get("session-id") {
            if canonical != alias {
                return Err(GatewayError::bad_request(
                    "Conflicting session identity headers",
                ));
            }
        } else {
            headers.insert("session-id", alias);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn native_session_alias_maps_and_deduplicates_equal_names() {
        for canonical in [false, true] {
            let mut headers = HeaderMap::new();
            headers.insert("session_id", HeaderValue::from_static("dsh-session"));
            if canonical {
                headers.insert("session-id", HeaderValue::from_static("dsh-session"));
            }
            normalize_headers(&mut headers).unwrap();
            assert_eq!(headers["session-id"], "dsh-session");
            assert!(!headers.contains_key("session_id"));
        }
    }

    #[test]
    fn native_session_alias_rejects_conflicting_duplicate_or_invalid_identity() {
        for kind in ["conflict", "duplicate", "empty", "large"] {
            let mut headers = HeaderMap::new();
            headers.insert("session_id", HeaderValue::from_static("session"));
            match kind {
                "conflict" => {
                    headers.insert("session-id", HeaderValue::from_static("other"));
                }
                "duplicate" => {
                    headers.append("session_id", HeaderValue::from_static("session"));
                }
                "empty" => {
                    headers.insert("session_id", HeaderValue::from_static(""));
                }
                _ => {
                    headers.insert(
                        "session_id",
                        HeaderValue::from_str(&"x".repeat(1025)).unwrap(),
                    );
                }
            }
            assert!(normalize_headers(&mut headers).is_err(), "{kind}");
        }
    }

    #[test]
    fn native_session_alias_preserves_canonical_and_missing_identity() {
        let mut headers = HeaderMap::new();
        normalize_headers(&mut headers).unwrap();
        assert!(headers.is_empty());
        headers.insert("session-id", HeaderValue::from_static("native"));
        normalize_headers(&mut headers).unwrap();
        assert_eq!(headers["session-id"], "native");
    }
}
