//! Session cookie application and ordered response-cookie merging.

use std::collections::HashMap;

use rquest::header::HeaderMap;

use crate::protocol::gemini_canvas;
use crate::upstream::common::insert_header_map_value;

pub(crate) fn apply_gemini_canvas_cookie_header(
    headers: &mut HeaderMap,
    session: &gemini_canvas::GeminiCanvasPureHttpSession,
) {
    insert_header_map_value(headers, "cookie", &session.cookie_header);
}

pub(crate) fn merge_gemini_canvas_cookie_header(
    existing: &str,
    set_cookie_values: &[String],
) -> String {
    let mut ordered_pairs: Vec<(String, String)> = existing
        .split(';')
        .filter_map(|segment| {
            let (name, value) = segment.trim().split_once('=')?;
            let name = name.trim();
            let value = value.trim();
            if name.is_empty() || value.is_empty() {
                return None;
            }
            Some((name.to_string(), value.to_string()))
        })
        .collect();
    let mut positions: HashMap<String, usize> = ordered_pairs
        .iter()
        .enumerate()
        .map(|(index, (name, _))| (name.clone(), index))
        .collect();

    for set_cookie in set_cookie_values {
        for logical_cookie in set_cookie
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            let Some((name, value)) = logical_cookie
                .split(';')
                .next()
                .and_then(|segment| segment.trim().split_once('='))
            else {
                continue;
            };
            let name = name.trim();
            let value = value.trim();
            if name.is_empty() || value.is_empty() {
                continue;
            }
            if let Some(index) = positions.get(name).copied() {
                ordered_pairs[index].1 = value.to_string();
            } else {
                positions.insert(name.to_string(), ordered_pairs.len());
                ordered_pairs.push((name.to_string(), value.to_string()));
            }
        }
    }

    ordered_pairs
        .into_iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join("; ")
}

pub(crate) fn apply_gemini_canvas_response_cookies(
    headers: &HeaderMap,
    session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
) {
    let set_cookie_values = headers
        .get_all(rquest::header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok().map(str::to_string))
        .collect::<Vec<_>>();
    if set_cookie_values.is_empty() {
        return;
    }

    let merged_cookie_header =
        merge_gemini_canvas_cookie_header(&session.cookie_header, &set_cookie_values);
    if !merged_cookie_header.is_empty() {
        session.cookie_header = merged_cookie_header;
    }

    for set_cookie in &set_cookie_values {
        for logical_cookie in set_cookie
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            let Some((name, value)) = logical_cookie
                .split(';')
                .next()
                .and_then(|segment| segment.trim().split_once('='))
            else {
                continue;
            };
            match name.trim() {
                "__Secure-1PAPISID" | "__Secure-3PAPISID" | "SAPISID"
                    if !value.trim().is_empty() =>
                {
                    session.sapisid = value.trim().to_string();
                    return;
                }
                _ => {}
            }
        }
    }
}
