//! Bind every snapshot to the initial generation; never silently drop malformed clips.
use super::transport::failure;
use crate::{
    error::GatewayError,
    protocol::suno::{self, SunoClip},
};
use serde_json::Value;
use std::collections::HashSet;

pub(super) fn snapshot(
    body: &Value,
    expected: Option<&[String]>,
) -> Result<Vec<SunoClip>, GatewayError> {
    let raw = body
        .get("clips")
        .and_then(Value::as_array)
        .ok_or_else(identity)?;
    // A/B today; keep a small bounded set without inventing a mandatory exact pair.
    if raw.len() > 16 || (raw.is_empty() && expected.is_none()) {
        return Err(identity());
    }
    let mut ids = HashSet::new();
    for item in raw {
        let id = item
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(identity)?;
        if id.is_empty()
            || id.len() > 128
            || id.trim() != id
            || !ids.insert(id)
            || expected.is_some_and(|set| !set.iter().any(|s| s == id))
        {
            return Err(identity());
        }
        if item.get("status").is_some_and(|value| {
            value
                .as_str()
                .is_none_or(|s| s.is_empty() || s.len() > 64 || s.trim() != s)
        }) {
            return Err(failure("suno_clip_status", "Suno 歌曲状态格式无效。"));
        }
    }
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    let mut clips = suno::extract_clips_from_feed(body)?;
    if clips.iter().any(|c| {
        matches!(
            c.status.as_str(),
            "error" | "failed" | "canceled" | "cancelled"
        )
    }) {
        return Err(failure(
            "suno_generation_failed",
            "Suno 歌曲任务失败；远端消息已省略，不自动重提。",
        ));
    }
    if let Some(expected) = expected {
        clips.sort_by_key(|c| {
            expected
                .iter()
                .position(|id| id == &c.id)
                .expect("validated clip identity")
        });
    }
    Ok(clips)
}
fn identity() -> GatewayError {
    failure(
        "suno_clip_identity",
        "Suno 歌曲标识缺失、重复、格式错误或不属于本次提交；不猜测成功。",
    )
}
