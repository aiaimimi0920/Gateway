use rquest::header::{HeaderMap, HeaderName, HeaderValue};

pub(crate) fn header_map_string(headers: &rquest::header::HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(crate) fn insert_runtime_header(headers: &mut HeaderMap, name: &'static str, value: &str) {
    if let Ok(header_value) = HeaderValue::from_str(value) {
        let header_name = HeaderName::from_static(name);
        headers.insert(header_name, header_value);
    }
}

pub(crate) fn extract_bearer_token(headers: &rquest::header::HeaderMap) -> Option<String> {
    header_map_string(headers, "authorization").and_then(|value| {
        value
            .strip_prefix("Bearer ")
            .or_else(|| value.strip_prefix("bearer "))
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .map(str::to_string)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_map_string_trims_values_and_drops_empty_entries() {
        let mut headers = HeaderMap::new();
        headers.insert("x-trimmed", HeaderValue::from_static("  value  "));
        headers.insert("x-empty", HeaderValue::from_static("   "));

        assert_eq!(
            header_map_string(&headers, "x-trimmed").as_deref(),
            Some("value")
        );
        assert_eq!(header_map_string(&headers, "x-empty"), None);
        assert_eq!(header_map_string(&headers, "missing"), None);
    }

    #[test]
    fn extract_bearer_token_accepts_bearer_prefix_case_insensitively() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_static("Bearer token-123"),
        );
        assert_eq!(extract_bearer_token(&headers).as_deref(), Some("token-123"));

        headers.insert(
            "authorization",
            HeaderValue::from_static("bearer token-456"),
        );
        assert_eq!(extract_bearer_token(&headers).as_deref(), Some("token-456"));
    }

    #[test]
    fn insert_runtime_header_sets_valid_values_and_skips_invalid_payload() {
        let mut headers = HeaderMap::new();
        insert_runtime_header(&mut headers, "referer", "https://example.com");
        assert_eq!(
            header_map_string(&headers, "referer").as_deref(),
            Some("https://example.com")
        );

        insert_runtime_header(&mut headers, "referer", "bad\r\nvalue");
        assert_eq!(
            header_map_string(&headers, "referer").as_deref(),
            Some("https://example.com")
        );
    }
}
