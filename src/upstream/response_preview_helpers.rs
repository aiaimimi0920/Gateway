pub(crate) fn truncate_response_preview(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        None => s,
        Some((idx, _)) => &s[..idx],
    }
}

pub(crate) fn compact_response_preview(body_text: &str, max_chars: usize) -> String {
    let normalized = body_text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return "<empty>".to_string();
    }
    if normalized.chars().count() > max_chars {
        return format!(
            "{}...(truncated)",
            truncate_response_preview(&normalized, max_chars)
        );
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_response_preview_preserves_char_boundaries() {
        assert_eq!(truncate_response_preview("hello", 10), "hello");
        assert_eq!(truncate_response_preview("你好世界", 3), "你好世");
    }

    #[test]
    fn compact_response_preview_normalizes_whitespace_and_marks_truncation() {
        assert_eq!(
            compact_response_preview(" \n  alpha   beta \t gamma ", 10),
            "alpha beta...(truncated)"
        );
        assert_eq!(compact_response_preview("   ", 10), "<empty>");
    }
}
