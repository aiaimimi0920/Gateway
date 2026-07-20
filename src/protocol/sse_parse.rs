// ---------------------------------------------------------------------------
// SSE frame parsing
//
// Parses Server-Sent Events (SSE) streams from upstream providers.
// Handles both standard "data: " and no-space "data:" variants.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// SseFrame
// ---------------------------------------------------------------------------

/// A fully-assembled SSE event frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseFrame {
    /// The `event:` field value, if present.
    pub event_name: Option<String>,
    /// The assembled `data` payload (multi-line data is joined with `\n`).
    pub data: String,
}

// ---------------------------------------------------------------------------
// SseParseState
// ---------------------------------------------------------------------------

/// Accumulator used between successive calls to [`parse_sse_line`].
///
/// Create one per upstream response stream and pass a mutable reference to
/// each call.
#[derive(Debug, Default)]
pub struct SseParseState {
    /// Current `event:` field for the frame being assembled.
    pub event_name: Option<String>,
    /// Accumulated `data:` lines for the frame being assembled.
    pub data_lines: Vec<String>,
}

impl SseParseState {
    /// Create a new, empty parse state.
    pub fn new() -> Self {
        Self::default()
    }
}

// ---------------------------------------------------------------------------
// parse_sse_line
// ---------------------------------------------------------------------------

/// Feed a single raw text line from an SSE byte stream into the parser.
///
/// Returns `Some(SseFrame)` when an empty line signals the end of an event
/// frame.  Returns `None` when the line is field data (still accumulating) or
/// a comment.
///
/// The caller should split the raw byte stream on `'\n'` and call this
/// function once per line, stripping trailing `'\r'` if present (CRLF is
/// valid in SSE).
///
/// `[DONE]` data payloads are treated as a terminal frame whose `data` field
/// is the literal string `"[DONE]"`.
pub fn parse_sse_line(line: &str, state: &mut SseParseState) -> Option<SseFrame> {
    // Strip trailing CR so both `\n` and `\r\n` line endings work.
    let line = line.trim_end_matches('\r');

    if line.is_empty() {
        // Empty line = end of frame.
        if state.data_lines.is_empty() && state.event_name.is_none() {
            // Nothing accumulated — skip.
            return None;
        }

        let frame = SseFrame {
            event_name: state.event_name.take(),
            data: state.data_lines.join("\n"),
        };
        state.data_lines.clear();
        return Some(frame);
    }

    // Comments (lines starting with `:`) are ignored per the SSE spec.
    if line.starts_with(':') {
        return None;
    }

    // Field: value — support both "field: value" and "field:value".
    let (field, value) = if let Some(colon_pos) = line.find(':') {
        let field = &line[..colon_pos];
        let value = &line[colon_pos + 1..];
        // Strip a single leading space if present (standard SSE).
        let value = value.strip_prefix(' ').unwrap_or(value);
        (field, value)
    } else {
        // Line with no colon is treated as field name with empty value.
        (line, "")
    };

    match field {
        "data" => {
            state.data_lines.push(value.to_string());
        }
        "event" => {
            state.event_name = Some(value.to_string());
        }
        "id" | "retry" => {
            // Acknowledged but not stored in our simplified state.
        }
        _ => {
            // Unknown fields are ignored per the spec.
        }
    }

    None
}

// ---------------------------------------------------------------------------
// format helpers
// ---------------------------------------------------------------------------

/// Format a string as an SSE `data:` line (with trailing double-newline).
pub fn format_sse_data(data: &str) -> String {
    format!("data: {data}\n\n")
}

/// Format a complete SSE event with optional event name.
///
/// If `event` is `Some`, emits both `event: <name>\n` and `data: <data>\n\n`.
/// If `event` is `None`, emits only `data: <data>\n\n`.
pub fn format_sse_event(event: Option<&str>, data: &str) -> String {
    match event {
        Some(name) => format!("event: {name}\ndata: {data}\n\n"),
        None => format!("data: {data}\n\n"),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_lines(lines: &[&str]) -> Vec<SseFrame> {
        let mut state = SseParseState::new();
        let mut frames = Vec::new();
        for &line in lines {
            if let Some(frame) = parse_sse_line(line, &mut state) {
                frames.push(frame);
            }
        }
        frames
    }

    // ── basic frame parsing ────────────────────────────────────────────────

    #[test]
    fn parse_single_data_frame() {
        let frames = parse_lines(&[r#"data: {"hello":"world"}"#, ""]);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].data, r#"{"hello":"world"}"#);
        assert_eq!(frames[0].event_name, None);
    }

    #[test]
    fn parse_data_without_space_after_colon() {
        // Some providers (like iFlow) omit the space after "data:".
        let frames = parse_lines(&[r#"data:{"hello":"world"}"#, ""]);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].data, r#"{"hello":"world"}"#);
    }

    #[test]
    fn parse_event_field_sets_event_name() {
        let frames = parse_lines(&["event: message", r#"data: hello"#, ""]);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].event_name.as_deref(), Some("message"));
        assert_eq!(frames[0].data, "hello");
    }

    #[test]
    fn parse_multi_line_data_joined_with_newline() {
        let frames = parse_lines(&["data: line1", "data: line2", "data: line3", ""]);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].data, "line1\nline2\nline3");
    }

    #[test]
    fn parse_multiple_frames() {
        let frames = parse_lines(&["data: first", "", "data: second", ""]);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].data, "first");
        assert_eq!(frames[1].data, "second");
    }

    #[test]
    fn empty_lines_between_frames_do_not_produce_empty_frames() {
        // Two consecutive empty lines — only one frame boundary.
        let frames = parse_lines(&["data: hello", "", "", "data: world", ""]);
        assert_eq!(frames.len(), 2);
    }

    #[test]
    fn comment_lines_are_ignored() {
        let frames = parse_lines(&[": this is a comment", "data: payload", ""]);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].data, "payload");
    }

    #[test]
    fn done_sentinel_becomes_frame() {
        let frames = parse_lines(&["data: [DONE]", ""]);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].data, "[DONE]");
    }

    #[test]
    fn crlf_line_endings_stripped() {
        let mut state = SseParseState::new();
        parse_sse_line("data: hello\r", &mut state);
        let frame = parse_sse_line("\r", &mut state);
        let frame = frame.unwrap();
        assert_eq!(frame.data, "hello");
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let frames = parse_lines(&["unknown_field: ignored", "data: real", ""]);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].data, "real");
    }

    // ── format helpers ────────────────────────────────────────────────────

    #[test]
    fn format_sse_data_adds_prefix_and_double_newline() {
        let s = format_sse_data(r#"{"key":"val"}"#);
        assert_eq!(s, "data: {\"key\":\"val\"}\n\n");
    }

    #[test]
    fn format_sse_event_with_name() {
        let s = format_sse_event(Some("update"), "payload");
        assert_eq!(s, "event: update\ndata: payload\n\n");
    }

    #[test]
    fn format_sse_event_without_name() {
        let s = format_sse_event(None, "payload");
        assert_eq!(s, "data: payload\n\n");
    }

    // ── state reset between frames ────────────────────────────────────────

    #[test]
    fn event_name_resets_after_frame() {
        let mut state = SseParseState::new();
        parse_sse_line("event: first_event", &mut state);
        parse_sse_line("data: first data", &mut state);
        let frame1 = parse_sse_line("", &mut state).unwrap();
        assert_eq!(frame1.event_name.as_deref(), Some("first_event"));

        // Second frame — no event field.
        parse_sse_line("data: second data", &mut state);
        let frame2 = parse_sse_line("", &mut state).unwrap();
        assert_eq!(frame2.event_name, None);
    }
}
