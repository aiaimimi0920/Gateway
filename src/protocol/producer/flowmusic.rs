//! FlowMusic uses the Producer wire model, with a human-readable Lyria selection in client context.
use crate::{error::GatewayError, protocol::canonical::CanonicalRelayRequest};
use serde_json::json;

pub fn prepare_flowmusic_request(
    req: &CanonicalRelayRequest,
) -> Result<CanonicalRelayRequest, GatewayError> {
    let mut request = req.clone();
    let body = request.raw_body.as_object_mut().ok_or_else(|| {
        GatewayError::bad_request("FlowMusic music request must be a JSON object.")
    })?;
    body.entry("client_context").or_insert_with(|| {
        json!({"current_song_id":null,"song_queue":[],"project_id":null,
        "selected_model":"Lyria 3.5","lyrics_id_map":{},"ghostwriter_version":"standard"})
    });
    Ok(request)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flowmusic_context_does_not_rewrite_prompts_or_legacy_producer_shape() {
        let original = crate::protocol::producer::normalize_music_generations(
            json!({"model":"producer:standard","prompt":"Original instrumental music"}),
        )
        .unwrap();
        let prepared = prepare_flowmusic_request(&original).unwrap();
        let body =
            crate::protocol::producer::build_send_message_request(&prepared, "producer:standard")
                .unwrap();
        assert_eq!(body["model_name"], "producer:standard");
        assert_eq!(body["client_context"]["selected_model"], "Lyria 3.5");
        assert!(original.raw_body.get("client_context").is_none());
        assert_eq!(body["parts"][0]["content"], "Original instrumental music");
    }
}
