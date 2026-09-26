use serde_json::Value;

pub(super) fn strip_xssi_prefix(body: &str) -> &str {
    body.trim_start()
        .strip_prefix(")]}'")
        .map(str::trim_start)
        .unwrap_or(body)
}

pub(super) fn parse_locator_response_envelopes(content: &str) -> Vec<Value> {
    let content = content.trim();
    if content.is_empty() {
        return Vec::new();
    }

    if let Ok(parsed) = serde_json::from_str::<Value>(content) {
        return match parsed {
            Value::Array(items) => items,
            other => vec![other],
        };
    }

    let mut collected = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(parsed) = serde_json::from_str::<Value>(line) {
            match parsed {
                Value::Array(items) => collected.extend(items),
                other => collected.push(other),
            }
        }
    }

    collected
}

pub(super) fn parse_response_frames(content: &str) -> (Vec<Value>, String) {
    let mut consumed_chars = 0usize;
    let mut frames = Vec::new();
    while consumed_chars < content.len() {
        let mut index = consumed_chars;
        while let Some(ch) = content[index..].chars().next() {
            if ch.is_whitespace() {
                index += ch.len_utf8();
            } else {
                break;
            }
            if index >= content.len() {
                break;
            }
        }
        if index >= content.len() {
            consumed_chars = index;
            break;
        }

        let mut digit_end = index;
        while let Some(ch) = content[digit_end..].chars().next() {
            if ch.is_ascii_digit() {
                digit_end += ch.len_utf8();
            } else {
                break;
            }
            if digit_end >= content.len() {
                break;
            }
        }
        if digit_end == index {
            break;
        }
        let line_ending = if content[digit_end..].starts_with("\r\n") {
            2
        } else if content[digit_end..].starts_with('\n') {
            1
        } else {
            break;
        };
        let length = match content[index..digit_end].parse::<usize>() {
            Ok(value) => value,
            Err(_) => break,
        };
        let start_content = digit_end + line_ending;
        let Some((end_pos, parsed)) = parse_response_frame_chunk(content, start_content, length)
        else {
            break;
        };
        consumed_chars = end_pos;
        match parsed {
            Value::Array(items) => frames.extend(items),
            other => frames.push(other),
        }
    }

    (frames, content[consumed_chars..].to_string())
}

fn parse_response_frame_chunk(
    content: &str,
    start: usize,
    expected_length: usize,
) -> Option<(usize, Value)> {
    for end in [
        utf16_end_index(content, start, expected_length),
        fallback_line_json_end_index(content, start),
    ]
    .into_iter()
    .flatten()
    {
        let chunk = content[start..end].trim();
        if chunk.is_empty() {
            continue;
        }
        if let Ok(parsed) = serde_json::from_str::<Value>(chunk) {
            return Some((end, parsed));
        }
    }

    None
}

fn fallback_line_json_end_index(content: &str, start: usize) -> Option<usize> {
    if start >= content.len() {
        return None;
    }
    let remainder = &content[start..];
    let line_end_offset = remainder.find('\n').unwrap_or(remainder.len());
    let mut end = start + line_end_offset;
    if end > start && content.as_bytes().get(end.wrapping_sub(1)) == Some(&b'\r') {
        end -= 1;
    }
    let chunk = content[start..end].trim();
    if chunk.is_empty() {
        return None;
    }
    serde_json::from_str::<Value>(chunk).ok().map(|_| end)
}

fn utf16_end_index(content: &str, start: usize, units: usize) -> Option<usize> {
    let mut consumed_units = 0usize;
    for (offset, ch) in content[start..].char_indices() {
        let width = ch.len_utf16();
        if consumed_units + width > units {
            break;
        }
        consumed_units += width;
        if consumed_units == units {
            return Some(start + offset + ch.len_utf8());
        }
    }
    None
}
