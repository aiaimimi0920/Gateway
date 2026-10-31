//! Admit only a complete XqA3Ic result; never scan private text or follow asset URLs.
use base64::Engine;
use serde_json::Value;

use super::speech::{
    invalid_speech_response, MAX_SPEECH_AUDIO_BYTES, MAX_SPEECH_WIRE_BYTES, SPEECH_RPC,
};
use super::speech_containers::{valid_ogg_opus, valid_wav};
use crate::error::GatewayError;

fn envelopes(body: &str) -> Option<Vec<Value>> {
    if body.len() > MAX_SPEECH_WIRE_BYTES {
        return None;
    }
    let mut rest = body.trim();
    rest = rest.strip_prefix(")]}'").unwrap_or(rest).trim_start();
    if rest.starts_with('[') {
        return serde_json::from_str(rest).ok();
    }
    let mut frames = Vec::new();
    while !rest.is_empty() {
        if frames.len() > 64 {
            return None;
        }
        let (length, content) = rest.split_once('\n')?;
        let units = length.trim_end_matches('\r').parse::<usize>().ok()?;
        if units == 0 || units > MAX_SPEECH_WIRE_BYTES {
            return None;
        }
        let mut consumed = 0usize;
        let mut end = None;
        for (offset, ch) in content.char_indices() {
            consumed += ch.len_utf16();
            if consumed == units {
                end = Some(offset + ch.len_utf8());
                break;
            }
            if consumed > units {
                return None;
            }
        }
        let end = end?;
        let rows: Vec<Value> = serde_json::from_str(&content[..end]).ok()?;
        frames.extend(rows);
        // Declared length is authoritative; a truncated frame cannot fall back to line scanning.
        rest = content[end..].trim_start();
    }
    Some(frames)
}

pub(crate) fn speech_audio(
    body: &str,
    requested: &str,
) -> Result<(Vec<u8>, &'static str), GatewayError> {
    let frames = envelopes(body).ok_or_else(invalid_speech_response)?;
    let mut result = None;
    for frame in frames {
        let Some(row) = frame.as_array() else {
            return Err(invalid_speech_response());
        };
        if row.get(1).and_then(Value::as_str) != Some(SPEECH_RPC) {
            continue;
        }
        if result.is_some()
            || row.len() != 7
            || row[0] != "wrb.fr"
            || row[3..6].iter().any(|v| !v.is_null())
            || row[6] != "generic"
        {
            return Err(invalid_speech_response());
        }
        let payload: Vec<Value> = row[2]
            .as_str()
            .and_then(|s| serde_json::from_str(s).ok())
            .ok_or_else(invalid_speech_response)?;
        if payload.len() != 1 {
            return Err(invalid_speech_response());
        }
        let encoded = payload[0].as_str().ok_or_else(invalid_speech_response)?;
        if encoded.len() > MAX_SPEECH_AUDIO_BYTES.div_ceil(3) * 4 {
            return Err(invalid_speech_response());
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|_| invalid_speech_response())?;
        if bytes.len() > MAX_SPEECH_AUDIO_BYTES {
            return Err(invalid_speech_response());
        }
        let mime = if requested == "wav" && valid_wav(&bytes) {
            "audio/wav"
        } else if matches!(requested, "ogg" | "opus") && valid_ogg_opus(&bytes) {
            "audio/ogg"
        } else {
            return Err(invalid_speech_response());
        };
        result = Some((bytes, mime));
    }
    result.ok_or_else(invalid_speech_response)
}
