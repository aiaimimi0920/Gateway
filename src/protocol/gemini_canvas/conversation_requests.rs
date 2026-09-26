use serde_json::{json, Value};

use super::batchexecute::build_text_batchexecute_request;
use crate::error::GatewayError;
use crate::protocol::gemini_web;

pub fn build_conversation_list_probe_request(
    bootstrap: &gemini_web::GeminiWebBootstrap,
    source_path: &str,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    build_text_batchexecute_request(
        "MaZiqc",
        json!([13, null, [1, null, 1]]),
        bootstrap,
        source_path,
    )
}

pub fn build_conversation_list_full_request(
    bootstrap: &gemini_web::GeminiWebBootstrap,
    source_path: &str,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    build_text_batchexecute_request(
        "MaZiqc",
        json!([13, null, [0, null, 1]]),
        bootstrap,
        source_path,
    )
}

pub fn build_video_job_poll_request(
    bootstrap: &gemini_web::GeminiWebBootstrap,
    source_path: &str,
    job_id: &str,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    build_text_batchexecute_request(
        "kwDCne",
        Value::Array(vec![Value::String(job_id.trim().to_string())]),
        bootstrap,
        source_path,
    )
}

pub fn build_video_completion_followup_request(
    bootstrap: &gemini_web::GeminiWebBootstrap,
    source_path: &str,
    conversation_id: &str,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    build_text_batchexecute_request(
        "hNvQHb",
        json!([conversation_id.trim(), 10, null, 1, [1], [4], null, 1]),
        bootstrap,
        source_path,
    )
}

pub fn build_video_metadata_followup_request(
    bootstrap: &gemini_web::GeminiWebBootstrap,
    source_path: &str,
    conversation_id: &str,
    response_id: &str,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    let conversation_id = conversation_id.trim();
    let response_id = response_id.trim();
    if conversation_id.is_empty() || response_id.is_empty() {
        return Err(GatewayError::server_error(
            "Gemini Canvas video metadata follow-up requires non-empty conversation_id and response_id.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_video_metadata_followup_missing_ids"));
    }

    build_text_batchexecute_request(
        "MUAZcd",
        json!([
            null,
            [["unread_metadata"]],
            [
                conversation_id,
                null,
                null,
                null,
                null,
                null,
                [[conversation_id, response_id], 0]
            ]
        ]),
        bootstrap,
        source_path,
    )
}
