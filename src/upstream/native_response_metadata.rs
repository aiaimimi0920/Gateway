//! Bounded native Responses state, never credentials, cookies or provider billing data.
use axum::http::HeaderMap;

#[derive(Default)]
pub(crate) struct NativeResponseMetadata(HeaderMap);

impl NativeResponseMetadata {
    pub(crate) fn read(headers: &HeaderMap) -> Self {
        let mut selected = HeaderMap::new();
        for name in ["x-codex-turn-state", "x-codex-routing-hint"] {
            // Ambiguous opaque state is not comma-joined or silently selected.
            let mut values = headers.get_all(name).iter();
            if let Some(value) = values.next() {
                if values.next().is_none()
                    && value
                        .to_str()
                        .is_ok_and(|value| crate::protocol::codex_wire::valid_header(name, value))
                {
                    selected.insert(name, value.clone());
                }
            }
        }
        Self(selected)
    }

    pub(crate) fn apply(self, target: &mut HeaderMap) {
        target.extend(self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn native_response_state_is_bounded_and_does_not_expose_account_secrets() {
        let mut headers = HeaderMap::new();
        headers.insert("x-codex-turn-state", HeaderValue::from_static("opaque"));
        headers.insert("x-codex-routing-hint", HeaderValue::from_static("route"));
        for name in [
            "set-cookie",
            "authorization",
            "x-codex-credits-balance",
            "connection",
        ] {
            headers.insert(name, HeaderValue::from_static("must-not-forward"));
        }
        let mut result = HeaderMap::new();
        NativeResponseMetadata::read(&headers).apply(&mut result);
        assert_eq!(result.len(), 2);
        assert_eq!(result["x-codex-turn-state"], "opaque");
        assert_eq!(result["x-codex-routing-hint"], "route");
        headers.append("x-codex-turn-state", HeaderValue::from_static("ambiguous"));
        headers.insert(
            "x-codex-routing-hint",
            HeaderValue::from_str(&"x".repeat(1025)).unwrap(),
        );
        let mut rejected = HeaderMap::new();
        NativeResponseMetadata::read(&headers).apply(&mut rejected);
        assert!(rejected.is_empty());
    }
}
