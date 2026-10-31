//! Complete length-prefixed envelopes only; no raw-text or recursive frame search.
use super::{invalid, GatewayError, IMAGE_READ_LIMIT};
use serde_json::Value;

pub(super) fn rows(body: &str) -> Result<Vec<Value>, GatewayError> {
    if body.len() > IMAGE_READ_LIMIT {
        return Err(invalid());
    }
    let mut rest = body.trim();
    rest = rest.strip_prefix(")]}'").unwrap_or(rest).trim_start();
    let mut result = Vec::new();
    if rest.starts_with('[') {
        result = serde_json::from_str(rest).map_err(|_| invalid())?;
    } else {
        let mut frame_count = 0;
        while !rest.is_empty() {
            frame_count += 1;
            if frame_count > 256 {
                return Err(invalid());
            }
            let (length, content) = rest.split_once('\n').ok_or_else(invalid)?;
            let units = length
                .trim_end_matches('\r')
                .parse::<usize>()
                .map_err(|_| invalid())?;
            if units == 0 || units > IMAGE_READ_LIMIT {
                return Err(invalid());
            }
            let mut consumed = 0;
            let mut end = None;
            for (offset, ch) in content.char_indices() {
                consumed += ch.len_utf16();
                if consumed == units {
                    end = Some(offset + ch.len_utf8());
                    break;
                }
                if consumed > units {
                    return Err(invalid());
                }
            }
            let end = end.ok_or_else(invalid)?;
            let rows: Vec<Value> = serde_json::from_str(&content[..end]).map_err(|_| invalid())?;
            if result.len().saturating_add(rows.len()) > 256 {
                return Err(invalid());
            }
            result.extend(rows);
            rest = content[end..].trim_start();
        }
    }
    if result.is_empty() || result.len() > 256 {
        return Err(invalid());
    }
    Ok(result)
}
