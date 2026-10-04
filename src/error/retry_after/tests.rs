use super::*;
use rquest::header::HeaderValue;
use std::time::Duration;

fn headers(entries: &[(&'static str, &str)]) -> HeaderMap {
    let mut headers = HeaderMap::new();
    for (name, value) in entries {
        headers.append(*name, HeaderValue::from_str(value).unwrap());
    }
    headers
}

fn parse(entries: &[(&'static str, &str)]) -> Option<u64> {
    retry_after_from_headers(&headers(entries), SystemTime::UNIX_EPOCH)
}

#[test]
fn seconds_and_millisecond_headers_are_normalized() {
    assert_eq!(parse(&[("retry-after", "2")]), Some(2_000));
    assert_eq!(parse(&[("retry-after", "0")]), Some(0));
    assert_eq!(parse(&[("retry-after-ms", "250")]), Some(250));
    assert_eq!(parse(&[("x-ms-retry-after-ms", " 350 ")]), Some(350));
}

#[test]
fn explicit_header_priority_and_invalid_fallback_are_stable() {
    assert_eq!(
        parse(&[
            ("retry-after", "9"),
            ("x-ms-retry-after-ms", "20"),
            ("retry-after-ms", "10")
        ]),
        Some(10)
    );
    assert_eq!(
        parse(&[
            ("retry-after-ms", "bad"),
            ("x-ms-retry-after-ms", "20"),
            ("retry-after", "9")
        ]),
        Some(20)
    );
    assert_eq!(
        parse(&[
            ("retry-after-ms", "-1"),
            ("x-ms-retry-after-ms", "bad"),
            ("retry-after", "9")
        ]),
        Some(9_000)
    );
}

#[test]
fn http_date_uses_an_injected_clock_and_accepts_legacy_http_forms() {
    let date = httpdate::parse_http_date("Sun, 06 Nov 1994 08:49:37 GMT").unwrap();
    let now = date - Duration::from_secs(2);
    for value in [
        "Sun, 06 Nov 1994 08:49:37 GMT",
        "Sunday, 06-Nov-94 08:49:37 GMT",
        "Sun Nov  6 08:49:37 1994",
    ] {
        assert_eq!(
            retry_after_from_headers(&headers(&[("retry-after", value)]), now),
            Some(2_000)
        );
    }
    assert_eq!(
        retry_after_from_headers(
            &headers(&[("retry-after", "Sun, 06 Nov 1994 08:49:37 GMT")]),
            date + Duration::from_secs(1)
        ),
        Some(0)
    );
}

#[test]
fn date_rounds_up_instead_of_retrying_early() {
    let date = httpdate::parse_http_date("Sun, 06 Nov 1994 08:49:37 GMT").unwrap();
    // Windows SystemTime uses 100 ns ticks; keep the sub-ms input representable.
    assert_eq!(
        retry_after_from_headers(
            &headers(&[("retry-after", "Sun, 06 Nov 1994 08:49:37 GMT")]),
            date - Duration::from_micros(1)
        ),
        Some(1)
    );
}

#[test]
fn malformed_missing_and_duplicate_hints_fall_back_without_panicking() {
    assert_eq!(parse(&[]), None);
    for value in [
        "", "-1", "+1", "1.5", "NaN", "Infinity", "1e3", "tomorrow", "1, 2",
    ] {
        assert_eq!(parse(&[("retry-after", value)]), None, "{value}");
    }
    assert_eq!(
        parse(&[
            ("retry-after-ms", "1"),
            ("retry-after-ms", "2"),
            ("retry-after", "3")
        ]),
        Some(3_000)
    );
    let mut invalid = HeaderMap::new();
    invalid.insert("retry-after", HeaderValue::from_bytes(b"\xff").unwrap());
    assert_eq!(
        retry_after_from_headers(&invalid, SystemTime::UNIX_EPOCH),
        None
    );
}

#[test]
fn overflow_and_oversize_cannot_turn_into_a_shorter_retry() {
    for value in ["18446744073709551615", "18446744073709551616"] {
        assert_eq!(parse(&[("retry-after", value)]), Some(u64::MAX));
    }
    assert_eq!(
        parse(&[("retry-after-ms", &"9".repeat(129)), ("retry-after", "1")]),
        Some(u64::MAX)
    );
    assert_eq!(
        parse(&[
            ("retry-after-ms", "18446744073709551616"),
            ("retry-after", "1")
        ]),
        Some(u64::MAX)
    );
}

#[test]
fn body_fractional_seconds_round_up_and_overflow_remains_terminally_large() {
    use crate::error::{classify_upstream_error, FallbackHint};
    for (value, expected) in [
        ("0.0001", 1),
        ("1.0001", 1_001),
        ("1e300", u64::MAX),
        ("1e400", u64::MAX),
        ("1e-400", 1),
        ("1.000000000000000000000001", 1_001),
        ("18446744073709551.615", u64::MAX),
        ("18446744073709551.614", u64::MAX - 1),
        ("1e999999999999999999999999999", u64::MAX),
        ("1e-999999999999999999999999999", 1),
        ("0e999999999999999999999999999", 0),
        ("-0.0", 0),
        ("-1e400", 5_000),
        ("\"1e400\"", 5_000),
        ("-1", 5_000),
    ] {
        let body = format!(r#"{{"retry_after":{value}}}"#);
        let error = classify_upstream_error(429, &body, None);
        assert!(
            matches!(error.fallback_hint, FallbackHint::Retry { delay_ms, .. } if delay_ms == expected),
            "{value}"
        );
    }
}

#[test]
fn body_hint_paths_preserve_priority_and_handle_nested_large_numbers() {
    use crate::error::{classify_upstream_error, FallbackHint};
    for (body, expected) in [
        (r#"{"error":{"retryAfter":1e400}}"#, u64::MAX),
        (r#"{"retryAfter":0.00001,"error":{"retry_after":9}}"#, 1),
        (
            r#"{"retry_after":2,"retryAfter":3,"error":{"retry_after":4}}"#,
            2_000,
        ),
        (
            r#"{"retry_after":"invalid","error":{"retry_after":4}}"#,
            5_000,
        ),
        (r#"{"retry_after":null,"error":{"retry_after":4}}"#, 5_000),
    ] {
        let error = classify_upstream_error(429, body, None);
        assert!(
            matches!(error.fallback_hint, FallbackHint::Retry { delay_ms, .. } if delay_ms == expected),
            "{body}"
        );
    }
}

#[test]
fn normalized_header_overrides_body_but_not_error_identity_or_auth_eligibility() {
    use crate::error::{classify_upstream_error, FallbackHint};
    let body = r#"{"error":{"message":"fixture","code":"original","retry_after":9}}"#;
    let error =
        classify_upstream_error(429, body, Some("fixture-provider")).with_retry_after_ms(Some(25));
    assert!(matches!(
        error.fallback_hint,
        FallbackHint::Retry { delay_ms: 25, .. }
    ));
    assert_eq!(error.message, "fixture");
    assert_eq!(error.code.as_deref(), Some("original"));
    assert_eq!(error.http_status, Some(429));
    assert_eq!(error.provider_name.as_deref(), Some("fixture-provider"));
    let error = classify_upstream_error(401, body, None).with_retry_after_ms(Some(25));
    assert!(!error.retryable);
    assert!(matches!(error.fallback_hint, FallbackHint::Abort { .. }));
}
