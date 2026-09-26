use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::GatewayError;

use super::{
    build_session_url, build_video_detail_path, build_video_library_path, build_video_status_path,
    extract_video_prompt, read_bool_fields, read_number_fields, read_string_fields,
};

pub fn build_video_proposal_prompt(body: &Value) -> Result<String, GatewayError> {
    let body_obj = body.as_object().ok_or_else(|| {
        GatewayError::bad_request("Producer.ai video request body must be a JSON object")
    })?;
    let creative_prompt = extract_video_prompt(body_obj).ok_or_else(|| {
        GatewayError::bad_request(
            "Producer.ai music video requests require one of: prompt, input, user_message, or args.user_message.",
        )
        .with_code("missing_video_prompt")
    })?;
    let aspect_ratio = read_string_fields(body_obj, &["aspect_ratio", "aspectRatio", "size"])
        .unwrap_or_else(|| "16:9".to_string());
    let style_image_url = read_string_fields(
        body_obj,
        &[
            "style_image_url",
            "styleImageUrl",
            "style_reference_image_url",
            "styleReferenceImageUrl",
        ],
    );
    let subject_image_url = read_string_fields(
        body_obj,
        &[
            "likeness_image_url",
            "likenessImageUrl",
            "subject_image_url",
            "subjectImageUrl",
        ],
    );
    let duration_seconds =
        read_number_fields(body_obj, &["duration_s", "durationSeconds", "duration"]).map(|value| {
            value
                .as_f64()
                .map(|entry| {
                    if entry.fract() == 0.0 {
                        format!("{entry:.0}")
                    } else {
                        format!("{entry}")
                    }
                })
                .unwrap_or_else(|| value.to_string())
        });
    let render_lyrics = read_bool_fields(
        body_obj,
        &[
            "render_lyrics",
            "renderLyrics",
            "display_lyrics",
            "displayLyrics",
        ],
    )
    .unwrap_or(false);

    Ok([
        Some("Please propose the music video.".to_string()),
        Some(format!("Vision: {creative_prompt}")),
        Some(format!("Use {aspect_ratio}.")),
        Some(match style_image_url {
            Some(url) => format!("Style reference image: {url}."),
            None => "Style reference image: none.".to_string(),
        }),
        Some(match subject_image_url {
            Some(url) => format!("Subject image: {url}."),
            None => "Subject image: none, generate one.".to_string(),
        }),
        Some(format!(
            "Lyrics on screen: {}.",
            if render_lyrics { "yes" } else { "no" }
        )),
        Some(match duration_seconds {
            Some(seconds) => format!("Duration: use about {seconds} seconds."),
            None => "Duration: choose the strongest section under 60 seconds.".to_string(),
        }),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" "))
}

pub fn choose_video_confirm_prompt(body: &Value, summary: &Value) -> Result<String, GatewayError> {
    let body_obj = body.as_object().ok_or_else(|| {
        GatewayError::bad_request("Producer.ai video request body must be a JSON object")
    })?;
    if let Some(explicit) = read_string_fields(body_obj, &["confirm_prompt", "confirmPrompt"]) {
        return Ok(explicit);
    }

    let proposed_inputs = summary
        .get("tool_calls")
        .and_then(|value| value.as_array())
        .and_then(|entries| {
            entries.iter().find_map(|entry| {
                let object = entry.as_object()?;
                let tool_name = object.get("tool_name")?.as_str()?.trim();
                if tool_name != "video__propose_music_video" {
                    return None;
                }
                object
                    .get("args")
                    .and_then(|value| value.as_object())
                    .and_then(|args| args.get("inputs"))
                    .and_then(|value| value.as_object())
                    .cloned()
            })
        });

    let start_seconds = proposed_inputs
        .as_ref()
        .and_then(|value| read_number_fields(value, &["start_s", "startSeconds", "start"]))
        .or_else(|| read_number_fields(body_obj, &["start_s", "startSeconds", "start"]));
    let duration_seconds = proposed_inputs
        .as_ref()
        .and_then(|value| read_number_fields(value, &["duration_s", "durationSeconds", "duration"]))
        .or_else(|| read_number_fields(body_obj, &["duration_s", "durationSeconds", "duration"]));
    let aspect_ratio = proposed_inputs
        .as_ref()
        .and_then(|value| read_string_fields(value, &["aspect_ratio", "aspectRatio", "size"]))
        .or_else(|| read_string_fields(body_obj, &["aspect_ratio", "aspectRatio", "size"]));
    let resolution = proposed_inputs
        .as_ref()
        .and_then(|value| read_string_fields(value, &["resolution"]))
        .or_else(|| read_string_fields(body_obj, &["resolution"]));

    if start_seconds.is_some() || duration_seconds.is_some() || aspect_ratio.is_some() {
        let mut lines = vec!["Create this exact proposed music video now.".to_string()];
        if let Some(value) = start_seconds {
            lines.push(format!(
                "Keep the current start time at {}s.",
                format_prompt_number(&value)
            ));
        }
        if let Some(value) = duration_seconds {
            lines.push(format!(
                "Keep the duration at {}s.",
                format_prompt_number(&value)
            ));
        }
        if let Some(value) = aspect_ratio {
            lines.push(format!("Keep the aspect ratio at {value}."));
        }
        if let Some(value) = resolution {
            lines.push(format!("Keep the resolution at {value}."));
        }
        lines.push(
            "Do not ask follow-up questions or change the selected song section.".to_string(),
        );
        return Ok(lines.join(" "));
    }

    let mut candidate_texts = Vec::new();
    if let Some(entries) = summary
        .get("suggestions")
        .and_then(|value| value.as_array())
    {
        for entry in entries {
            if let Some(text) = entry
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                candidate_texts.push(text.to_string());
            }
        }
    }
    if let Some(entries) = summary
        .get("message_texts")
        .and_then(|value| value.as_array())
    {
        for entry in entries {
            if let Some(text) = entry
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                candidate_texts.push(text.to_string());
            }
        }
    }

    let preferred = candidate_texts
        .iter()
        .find(|entry| {
            let lower = entry.to_ascii_lowercase();
            lower.contains("create the video")
                || lower.contains("create music video")
                || lower.contains("create video")
        })
        .cloned()
        .or_else(|| {
            candidate_texts
                .iter()
                .find(|entry| entry.to_ascii_lowercase().contains("render"))
                .cloned()
        })
        .or_else(|| {
            candidate_texts
                .iter()
                .find(|entry| entry.to_ascii_lowercase().contains("create"))
                .cloned()
        })
        .or_else(|| {
            candidate_texts
                .iter()
                .find(|entry| entry.to_ascii_lowercase().contains("start"))
                .cloned()
        });

    Ok(preferred.unwrap_or_else(|| "Create the video".to_string()))
}

#[allow(clippy::too_many_arguments)]
pub fn build_video_generation_response(
    base_url: &str,
    request_body: &Value,
    model: &str,
    clip_id: &str,
    conversation_id: &str,
    bootstrap_job_id: &str,
    creative_job_id: &str,
    confirmation_job_id: Option<&str>,
    video_job_id: &str,
    final_url: Option<&str>,
    final_status: &str,
    status_payload: &Value,
    creative_summary: &Value,
    confirmation_summary: Option<&Value>,
) -> Value {
    let completed = final_status == "completed" && final_url.is_some();
    let prompt = build_video_proposal_prompt(request_body).unwrap_or_else(|_| model.to_string());

    json!({
        "object": "video.generation",
        "provider": "producer.ai",
        "created": SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        "accepted": !completed,
        "completed": completed,
        "model": model,
        "prompt": prompt,
        "clip_id": clip_id,
        "conversation_id": conversation_id,
        "bootstrap_job_id": bootstrap_job_id,
        "creative_job_id": creative_job_id,
        "confirmation_job_id": confirmation_job_id,
        "job_id": video_job_id,
        "progress_url": build_session_url(base_url, conversation_id),
        "status_path": build_video_status_path(video_job_id),
        "detail_path": build_video_detail_path(video_job_id),
        "library_path": build_video_library_path(),
        "data": [{
            "kind": "video",
            "url": final_url,
            "mime_type": "video/mp4",
            "preview_url": status_payload
                .get("preview")
                .and_then(|value| value.get("video"))
                .cloned()
                .unwrap_or(Value::Null),
            "aspect_ratio": request_body
                .get("aspect_ratio")
                .or_else(|| request_body.get("aspectRatio"))
                .cloned()
                .unwrap_or(Value::Null),
            "resolution": request_body.get("resolution").cloned().unwrap_or(Value::Null),
            "duration_seconds": request_body
                .get("duration_s")
                .or_else(|| request_body.get("durationSeconds"))
                .or_else(|| request_body.get("duration"))
                .cloned()
                .unwrap_or(Value::Null),
        }],
        "state": final_status,
        "status": status_payload,
        "creative_stream": creative_summary,
        "confirmation_stream": confirmation_summary,
    })
}

fn format_prompt_number(value: &Value) -> String {
    value
        .as_f64()
        .map(|entry| {
            if entry.fract() == 0.0 {
                format!("{entry:.0}")
            } else {
                format!("{entry}")
            }
        })
        .unwrap_or_else(|| value.to_string())
}
