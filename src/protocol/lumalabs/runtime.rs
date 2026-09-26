//! LumaLabs runtime material and credential field parsing.

use super::DEFAULT_AUDIO_ARTIFACT_FIELD;
use super::DEFAULT_IMAGE_ARTIFACT_FIELD;
use super::DEFAULT_VIDEO_ARTIFACT_FIELD;
use crate::error::GatewayError;
use crate::routing::candidate::ProviderAccountPayload;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LumalabsRuntime {
    pub realm_id: String,
    pub image_action_type: Option<String>,
    pub video_action_type: Option<String>,
    pub audio_action_type: Option<String>,
    pub image_artifact_field: String,
    pub video_artifact_field: String,
    pub audio_artifact_field: String,
}

pub fn runtime_from_payload(
    payload: &ProviderAccountPayload,
) -> Result<LumalabsRuntime, GatewayError> {
    let extra = payload.extra_body.as_ref().ok_or_else(|| {
        GatewayError::bad_request(
            "LumaLabs credentials require extra_body runtime material (realmId).",
        )
        .with_code("missing_lumalabs_runtime")
    })?;

    let realm_id =
        read_required_hash_string(extra, &["realmId", "realm_id", "boardId", "board_id"])?;
    Ok(LumalabsRuntime {
        realm_id,
        image_action_type: read_optional_hash_string(
            extra,
            &[
                "imageActionType",
                "image_action_type",
                "imagesActionType",
                "images_action_type",
            ],
        ),
        video_action_type: read_optional_hash_string(
            extra,
            &[
                "videoActionType",
                "video_action_type",
                "videosActionType",
                "videos_action_type",
            ],
        ),
        audio_action_type: read_optional_hash_string(
            extra,
            &[
                "audioActionType",
                "audio_action_type",
                "musicActionType",
                "music_action_type",
            ],
        ),
        image_artifact_field: read_optional_hash_string(
            extra,
            &[
                "imageArtifactField",
                "image_artifact_field",
                "imagesArtifactField",
                "images_artifact_field",
            ],
        )
        .unwrap_or_else(|| DEFAULT_IMAGE_ARTIFACT_FIELD.to_string()),
        video_artifact_field: read_optional_hash_string(
            extra,
            &[
                "videoArtifactField",
                "video_artifact_field",
                "videosArtifactField",
                "videos_artifact_field",
            ],
        )
        .unwrap_or_else(|| DEFAULT_VIDEO_ARTIFACT_FIELD.to_string()),
        audio_artifact_field: read_optional_hash_string(
            extra,
            &[
                "audioArtifactField",
                "audio_artifact_field",
                "musicArtifactField",
                "music_artifact_field",
            ],
        )
        .unwrap_or_else(|| DEFAULT_AUDIO_ARTIFACT_FIELD.to_string()),
    })
}

fn read_optional_hash_string(
    map: &std::collections::HashMap<String, Value>,
    keys: &[&str],
) -> Option<String> {
    for key in keys {
        if let Some(value) = map.get(*key).and_then(|v| v.as_str()) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn read_required_hash_string(
    map: &std::collections::HashMap<String, Value>,
    keys: &[&str],
) -> Result<String, GatewayError> {
    for key in keys {
        if let Some(value) = map.get(*key).and_then(|v| v.as_str()) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Ok(trimmed.to_string());
            }
        }
    }

    Err(
        GatewayError::bad_request(format!("Missing required runtime field `{}`.", keys[0]))
            .with_code("missing_lumalabs_runtime_field"),
    )
}
