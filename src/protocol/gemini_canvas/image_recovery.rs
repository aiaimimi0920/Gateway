//! Read only the candidate bound by this generation; never discover the latest artwork.
use serde_json::{json, Value};

use super::{media_candidates::extract_media_assets_from_candidate_data, *};
use crate::error::FallbackHint;

#[cfg(test)]
#[path = "image_recovery/tests.rs"]
mod tests;
#[path = "image_recovery/wire.rs"]
mod wire;

pub(crate) const IMAGE_READ_RPC: &str = "hNvQHb";
pub(crate) const IMAGE_READ_PATH: &str = "/_/BardChatUi/data/batchexecute";
pub(crate) const IMAGE_READ_LIMIT: usize = 8 * 1024 * 1024;

// These identifiers are intentionally not Debug/Serialize: they belong to private history.
#[derive(PartialEq, Eq)]
pub(crate) struct AcceptedImageTurn {
    conversation: String,
    response: String,
    candidate: String,
}

pub(crate) fn image_read_error(code: &'static str) -> GatewayError {
    let mut error = GatewayError::service_unavailable(
        "Gemini Web image result could not be recovered; no generation was replayed and private response content was omitted.",
    )
    .with_provider("gemini_web_reverse_modular_compatible")
    .with_code(code);
    error.retryable = false;
    error.fallback_hint = FallbackHint::Abort {
        reason: "image_read_only_recovery".into(),
    };
    error
}

fn invalid() -> GatewayError {
    image_read_error("gemini_web_image_read_invalid")
}

fn id(value: &Value, prefix: &str) -> Option<String> {
    let value = value.as_str()?;
    let suffix = value.strip_prefix(prefix)?;
    (!suffix.is_empty()
        && value.len() <= 128
        && suffix
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-')))
    .then(|| value.to_string())
}

pub(crate) fn accepted_image_turn(body: &str) -> Result<AcceptedImageTurn, GatewayError> {
    let mut accepted = None;
    let mut metadata = None;
    for row in wire::rows(body)? {
        let row = row.as_array().ok_or_else(invalid)?;
        match row.first().and_then(Value::as_str) {
            Some("di" | "af.httprm") => continue,
            Some("wrb.fr") => {}
            _ => return Err(invalid()),
        }
        // StreamGenerate has no RPC id. A history/RPC envelope cannot authorize recovery.
        if row.get(1).is_none_or(|v| !v.is_null()) {
            return Err(invalid());
        }
        let value: Value = row
            .get(2)
            .and_then(Value::as_str)
            .and_then(|v| serde_json::from_str(v).ok())
            .ok_or_else(invalid)?;
        if row.get(5).is_some_and(|v| !v.is_null()) {
            return Err(invalid());
        }
        let Some(meta) = value.get(1).and_then(Value::as_array) else {
            continue;
        };
        let pair = (
            id(meta.first().unwrap_or(&Value::Null), "c_"),
            id(meta.get(1).unwrap_or(&Value::Null), "r_"),
        );
        let (Some(conversation), Some(response)) = pair else {
            return Err(invalid());
        };
        if metadata
            .as_ref()
            .is_some_and(|m| m != &(conversation.clone(), response.clone()))
        {
            return Err(invalid());
        }
        metadata = Some((conversation.clone(), response.clone()));
        let Some(candidates) = value.get(4).and_then(Value::as_array) else {
            continue;
        };
        if candidates.len() != 1 {
            return Err(invalid());
        }
        let candidate = id(&candidates[0][0], "rc_").ok_or_else(invalid)?;
        let turn = AcceptedImageTurn {
            conversation,
            response,
            candidate,
        };
        if accepted.as_ref().is_some_and(|previous| previous != &turn) {
            return Err(invalid());
        }
        accepted = Some(turn);
    }
    accepted.ok_or_else(|| image_read_error("gemini_web_image_read_missing_binding"))
}

pub(crate) fn image_history_request(
    turn: &AcceptedImageTurn,
    bootstrap: &gemini_web::GeminiWebBootstrap,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    // LIST_CONVERSATION_TURNS only: no prompt, write RPC, refresh or model selection.
    build_text_batchexecute_request(
        IMAGE_READ_RPC,
        json!([turn.conversation, 10, null, 1, [1], [4], null, 1]),
        bootstrap,
        "/app",
    )
}

pub(crate) fn recovered_image_assets(
    body: &str,
    turn: &AcceptedImageTurn,
) -> Result<Option<Vec<GeminiCanvasMediaAsset>>, GatewayError> {
    let mut payload = None;
    for row in wire::rows(body)? {
        let row = row.as_array().ok_or_else(invalid)?;
        match row.first().and_then(Value::as_str) {
            Some("di" | "af.httprm") => continue,
            Some("wrb.fr") => {}
            _ => return Err(invalid()),
        }
        if row.get(1).and_then(Value::as_str) != Some(IMAGE_READ_RPC)
            || payload.is_some()
            || row.len() != 7
            || row[3..6].iter().any(|v| !v.is_null())
            || row[6] != "generic"
        {
            return Err(invalid());
        }
        payload = Some(
            row[2]
                .as_str()
                .and_then(|v| serde_json::from_str::<Value>(v).ok())
                .ok_or_else(invalid)?,
        );
    }
    let payload = payload.ok_or_else(invalid)?;
    let turns = payload[0].as_array().ok_or_else(invalid)?;
    if turns.len() > 10 {
        return Err(invalid());
    }
    let mut target = None;
    for item in turns {
        if item[0][1].as_str() != Some(turn.response.as_str()) {
            continue;
        }
        if target.is_some() {
            return Err(invalid());
        }
        // The request is CID-scoped; reject a contradictory CID if the row supplies it.
        if item[0][0]
            .as_str()
            .is_some_and(|cid| cid != turn.conversation)
        {
            return Err(invalid());
        }
        target = Some(item);
    }
    let Some(target) = target else {
        return Ok(None);
    };
    let candidates = target[3][0].as_array().ok_or_else(invalid)?;
    if candidates.len() > 16 {
        return Err(invalid());
    }
    let mut found = None;
    for candidate in candidates {
        if candidate[0].as_str() != Some(turn.candidate.as_str()) {
            continue;
        }
        if found.is_some() {
            return Err(invalid());
        }
        found = Some(candidate);
    }
    let Some(candidate) = found else {
        return Ok(None);
    };
    match candidate[8][0].as_i64() {
        Some(1) => return Ok(None),
        Some(2) => {}
        _ => return Err(invalid()),
    }
    let Some(assets) =
        extract_media_assets_from_candidate_data(candidate, GeminiCanvasMediaOperation::Image)
    else {
        return Ok(None);
    };
    if assets.len() > 8 || assets.iter().any(|asset| !safe_image_url(&asset.url)) {
        return Err(invalid());
    }
    Ok(Some(assets))
}

fn safe_image_url(value: &str) -> bool {
    value.len() <= 8192
        && url::Url::parse(value).ok().is_some_and(|url| {
            url.scheme() == "https"
                && url.username().is_empty()
                && url.password().is_none()
                && url.fragment().is_none()
                && url.port().is_none()
                && url.host_str().is_some_and(|host| {
                    host == "googleusercontent.com" || host.ends_with(".googleusercontent.com")
                })
        })
}
