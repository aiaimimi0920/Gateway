mod persistence;
mod session;

use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::GatewayError;

pub(crate) use persistence::persist_gemini_canvas_program_runtime_material;
pub(crate) use session::{
    gemini_canvas_http_origin, gemini_canvas_page_base_url,
    gemini_canvas_pure_http_session_from_payload_or_storage, origin_from_url,
};

pub(crate) fn current_unix_timestamp_i64() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

pub(crate) fn gemini_canvas_signaler_zx_token() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

pub(crate) fn gemini_canvas_runtime_api_missing_google_api_key_error(
    page_harvest_probes: &[String],
) -> GatewayError {
    let mut error = GatewayError::unauthorized(
        "Gemini Canvas runtime API media lane requires a Google API key harvested from the Canvas session state.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_runtime_api_missing_google_api_key");
    if !page_harvest_probes.is_empty() {
        error.message = format!(
            "{}; page_harvest={}",
            error.message,
            page_harvest_probes.join(" | ")
        );
    }
    error
}

#[cfg(test)]
mod tests;
