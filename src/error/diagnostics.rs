//! Provider message redaction and UTF-8-safe diagnostic truncation.

use regex::Regex;
use std::sync::OnceLock;

/// Maximum number of characters retained in an upstream/provider error
/// message.  Error messages can cross API, log, and database boundaries, so
/// they must have a deterministic upper bound even when a provider returns a
/// very large response body.
pub const PROVIDER_ERROR_MESSAGE_MAX_CHARS: usize = 512;

const REDACTED_PROVIDER_VALUE: &str = "[REDACTED]";

fn provider_sensitive_header_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(
            r#"(?i)\b(authorization|proxy-authorization|cookie|set-cookie)\b["']?\s*[:=]\s*(?:"[^"]*"|'[^'\r\n]*'|[^\r\n,}]+)"#,
        )
        .expect("provider sensitive header regex must compile")
    })
}

fn provider_auth_scheme_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r#"(?i)\b(bearer|basic)\s+[^\s,;}\]\)"']+"#)
            .expect("provider auth scheme regex must compile")
    })
}

fn provider_jwt_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r#"\beyJ[A-Za-z0-9_-]{2,}\.[A-Za-z0-9_-]{2,}\.[A-Za-z0-9_-]{2,}\b"#)
            .expect("provider JWT regex must compile")
    })
}

fn provider_prefixed_key_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r#"(?i)\bsk-[A-Za-z0-9_-]{6,}\b"#)
            .expect("provider prefixed key regex must compile")
    })
}

fn provider_assignment_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(
            r#"(?i)\b(x[-_ ]?api[-_ ]?key|api[-_ ]?key|apikey|access[-_ ]?token|refresh[-_ ]?token|id[-_ ]?token|auth[-_ ]?token|token|session(?:[-_ ]?(?:id|token|key))?|secret(?:[-_ ]?key)?)\b["']?\s*[:=]\s*(?:"[^"]*"|'[^'\r\n]*'|[^\s,;&}\)]+)"#,
        )
        .expect("provider assignment regex must compile")
    })
}

/// Sanitize a provider/upstream error message before it crosses an external
/// or durable boundary. Classification must happen on the raw body first;
/// this helper only changes the human-readable message and therefore keeps
/// the original error kind/code decisions intact.
pub fn sanitize_provider_error_message(message: &str) -> String {
    let mut sanitized = provider_sensitive_header_pattern()
        .replace_all(message, |captures: &regex::Captures<'_>| {
            format!("{}: {REDACTED_PROVIDER_VALUE}", &captures[1])
        })
        .into_owned();
    sanitized = provider_auth_scheme_pattern()
        .replace_all(&sanitized, |captures: &regex::Captures<'_>| {
            format!("{} {REDACTED_PROVIDER_VALUE}", &captures[1])
        })
        .into_owned();
    sanitized = provider_jwt_pattern()
        .replace_all(&sanitized, REDACTED_PROVIDER_VALUE)
        .into_owned();
    sanitized = provider_prefixed_key_pattern()
        .replace_all(&sanitized, REDACTED_PROVIDER_VALUE)
        .into_owned();
    sanitized = provider_assignment_pattern()
        .replace_all(&sanitized, |captures: &regex::Captures<'_>| {
            format!("{}={REDACTED_PROVIDER_VALUE}", &captures[1])
        })
        .into_owned();

    // Remove control characters so a provider cannot inject additional log
    // lines or malformed response framing through an error message.
    let sanitized: String = sanitized
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect();
    let sanitized = sanitized.trim();
    if sanitized.chars().count() <= PROVIDER_ERROR_MESSAGE_MAX_CHARS {
        return sanitized.to_string();
    }

    let suffix = "...";
    let prefix_limit = PROVIDER_ERROR_MESSAGE_MAX_CHARS.saturating_sub(suffix.len());
    format!("{}{}", truncate(sanitized, prefix_limit).trim_end(), suffix)
}

pub(super) fn truncate(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        None => s,
        Some((idx, _)) => &s[..idx],
    }
}
