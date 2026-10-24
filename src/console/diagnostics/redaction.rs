//! Sanitize every occurrence before exporting logs; never truncate before secret recognition.
use regex::Regex;
use std::sync::OnceLock;

pub fn sanitize(text: &str) -> String {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    let patterns = PATTERNS.get_or_init(|| [
        r#"(?i)\b(?:authorization|proxy-authorization|cookie|set-cookie)\b["']?\s*[:=]\s*(?:"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|[^\r\n]+)"#,
        r#"(?i)\b(?:bearer|basic)\s+[^\s,;}\]\)"']+"#,
        r#"(?i)\b[A-Za-z0-9_.-]*(?:token|key|secret|password|passwd)[A-Za-z0-9_.-]*\b["']?\s*[:=]\s*(?:"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|[^\s,;&}\)]+)"#,
        r#"\beyJ[A-Za-z0-9_-]{2,}\.[A-Za-z0-9_-]{2,}\.[A-Za-z0-9_-]{2,}\b"#,
        r#"(?i)\bsk-[A-Za-z0-9_-]{6,}\b"#,
    ].iter().map(|pattern| Regex::new(pattern).expect("log redaction regex")).collect());
    let mut sanitized = text.to_owned();
    for pattern in patterns {
        sanitized = pattern.replace_all(&sanitized, "[REDACTED]").into_owned();
    }
    static URLS: OnceLock<Regex> = OnceLock::new();
    sanitized = URLS
        .get_or_init(|| Regex::new(r#"[A-Za-z][A-Za-z0-9+.-]*://[^\s<>"']+"#).unwrap())
        .replace_all(&sanitized, |captures: &regex::Captures<'_>| {
            crate::console::secrets::redact_url_value(&captures[0])
        })
        .into_owned();
    sanitized
        .chars()
        .map(|character| {
            if character.is_control() && character != '\n' && character != '\t' {
                ' '
            } else {
                character
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn removes_repeated_quoted_and_url_secrets() {
        let output = sanitize(
            r#"token=first token=second {"password":"third"} Bearer fourth Bearer fifth https://user:sixth@example.com/a?token=seventh sk-eighthsecret"#,
        );
        for secret in [
            "first",
            "second",
            "third",
            "fourth",
            "fifth",
            "sixth",
            "seventh",
            "eighthsecret",
        ] {
            assert!(!output.contains(secret), "{secret}: {output}");
        }
    }
    #[test]
    fn handles_non_http_urls_and_escaped_json_values() {
        let output = sanitize(
            r#"redis://user:redis-secret@host/0 postgres://user:db-secret@host/db {"password":"prefix\"tail-secret"} token='first\'tail-token'"#,
        );
        for secret in [
            "redis-secret",
            "db-secret",
            "prefix",
            "tail-secret",
            "tail-token",
        ] {
            assert!(!output.contains(secret), "{secret}: {output}");
        }
    }
}
