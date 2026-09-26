use super::*;
use regex::Regex;
use std::sync::OnceLock;

pub(super) fn apply_text_mode(
    text: &str,
    text_mode: &str,
    max_text_chars: usize,
) -> AppliedTextMode {
    let normalized = text.trim();
    if normalized.is_empty() || text_mode == "none" {
        return AppliedTextMode {
            text: None,
            truncated: false,
        };
    }
    let candidate = if text_mode == "preview_redacted" {
        redact_sensitive_text(normalized)
    } else {
        normalized.to_string()
    };
    let truncated = truncate_text(&candidate, max_text_chars);
    AppliedTextMode {
        text: Some(truncated.0),
        truncated: truncated.1,
    }
}

fn truncate_text(text: &str, max_text_chars: usize) -> (String, bool) {
    let char_count = text.chars().count();
    if char_count <= max_text_chars {
        return (text.to_string(), false);
    }
    (text.chars().take(max_text_chars).collect(), true)
}

pub(super) fn truncate_error_message(text: &str, max_chars: usize) -> String {
    truncate_text(text, max_chars).0
}

pub(super) fn redact_sensitive_text(text: &str) -> String {
    let mut result = text.to_string();
    result = email_regex()
        .replace_all(&result, "[REDACTED_EMAIL]")
        .into_owned();
    result = platform_api_key_regex()
        .replace_all(&result, "[REDACTED_API_KEY]")
        .into_owned();
    result = secret_key_regex()
        .replace_all(&result, "[REDACTED_SECRET]")
        .into_owned();
    result = bearer_regex()
        .replace_all(&result, "$1 [REDACTED_TOKEN]")
        .into_owned();
    result = query_secret_regex()
        .replace_all(&result, "$1[REDACTED]")
        .into_owned();
    result = field_secret_regex()
        .replace_all(&result, "$1 [REDACTED]")
        .into_owned();
    result
}

fn email_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?i)\b[A-Z0-9._%+\-]+@[A-Z0-9.\-]+\.[A-Z]{2,}\b").expect("email regex")
    })
}

fn platform_api_key_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"\b(?:new_api|neuro|nl_bundle_[A-Za-z0-9\-]+|nl_(?:tk|tm|rq)_[A-Za-z0-9\-]+)_[A-Za-z0-9._\-]+\b")
            .expect("platform api regex")
    })
}

fn secret_key_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\bsk-[A-Za-z0-9_\-]{12,}\b").expect("secret regex"))
}

fn bearer_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?i)\b(Bearer)\s+[A-Za-z0-9._=\-]{12,}\b").expect("bearer regex")
    })
}

fn query_secret_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?i)([?&](?:token|key|auth|signature|sig|password)=)[^&\s]+")
            .expect("query secret regex")
    })
}

fn field_secret_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?i)\b((?:token|secret|api[_-]?key|authorization)\s*[:=])\s*[^\s,;]+")
            .expect("field secret regex")
    })
}
