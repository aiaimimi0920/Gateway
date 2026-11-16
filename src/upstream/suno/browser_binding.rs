//! Basic browser tasks carry the routed native model, never the page's previous model selection.
use crate::{
    error::GatewayError,
    protocol::{canonical::CanonicalRelayRequest, suno},
    upstream::browser_worker_types::SunoBrowserWorkerSuccess,
};
use serde_json::{json, Value};

pub(super) fn prepare(
    request: &CanonicalRelayRequest,
    model: &str,
) -> Result<CanonicalRelayRequest, GatewayError> {
    let wire_model = suno::resolve_model(model)?;
    let body = request.raw_body.as_object().ok_or_else(invalid)?;
    // This worker drives the simple composer only. Do not silently ignore custom lyrics,
    // continuation, uploads, count overrides or caller-supplied native model fields.
    if body.keys().any(|key| {
        !matches!(
            key.as_str(),
            "model"
                | "prompt"
                | "input"
                | "parts"
                | "stream"
                | "n"
                | "response_format"
                | "user"
                | "session_key"
                | "sessionKey"
                | "wait_completion"
                | "waitCompletion"
                | "wait_audio"
                | "waitAudio"
                | "wait_timeout_secs"
                | "waitTimeoutSecs"
                | "timeout_secs"
                | "timeoutSecs"
                | "poll_interval_ms"
                | "pollIntervalMs"
                | "poll_secs"
                | "pollSecs"
        )
    }) || body
        .get("model")
        .is_some_and(|value| value.as_str().map(str::trim) != request.requested_model.as_deref())
        || body
            .get("stream")
            .is_some_and(|value| value != &Value::Bool(false))
        || body.get("n").is_some_and(|value| value.as_u64() != Some(1))
    {
        return Err(invalid());
    }
    let wait = suno::wait_for_completion(request);
    if [
        "wait_completion",
        "waitCompletion",
        "wait_audio",
        "waitAudio",
    ]
    .iter()
    .any(|key| {
        body.get(*key)
            .is_some_and(|value| value.as_bool() != Some(wait))
    }) {
        return Err(invalid());
    }
    let mut prepared = request.clone();
    prepared.raw_body = json!({"model":wire_model,"prompt":suno::prompt_from_request(request)?,"waitCompletion":wait});
    Ok(prepared)
}

pub(super) fn worker_model(body: &Value) -> Result<&str, GatewayError> {
    let model = body
        .get("model")
        .and_then(Value::as_str)
        .ok_or_else(invalid)?;
    if suno::resolve_model(model)? != model {
        return Err(invalid());
    }
    Ok(model)
}

pub(super) fn verify(
    result: &SunoBrowserWorkerSuccess,
    expected: &str,
) -> Result<(), GatewayError> {
    if result.wire_model.as_deref() != Some(expected) {
        return Err(GatewayError::server_error(
            "Suno browser result has no matching native model binding receipt; generation is not replayed.",
        )
        .with_provider("suno_compatible")
        .with_code("suno_browser_model_mismatch"));
    }
    Ok(())
}
fn invalid() -> GatewayError {
    GatewayError::bad_request(
        "Suno browser execution supports a basic simple task with an explicit routed native model only.",
    )
    .with_provider("suno_compatible")
    .with_code("suno_browser_request_mismatch")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn browser_binding_resolves_alias_without_mutating_the_original_request() {
        let request = suno::normalize_music_generations(json!({
            "model":"public-alias","prompt":"synthetic fixture","waitCompletion":false}))
        .unwrap();
        let prepared = prepare(&request, "chirp-v3-0").unwrap();
        assert_eq!(
            prepared.raw_body,
            json!({"model":"chirp-auk","prompt":"synthetic fixture","waitCompletion":false})
        );
        assert_eq!(request.raw_body["model"], "public-alias");
        assert_eq!(worker_model(&prepared.raw_body).unwrap(), "chirp-auk");
    }
    #[test]
    fn browser_binding_rejects_unimplemented_or_conflicting_task_options() {
        for extra in [
            json!({"mv":"chirp-auk"}),
            json!({"lyrics":"custom"}),
            json!({"continue_clip_id":"other"}),
            json!({"make_instrumental":true}),
            json!({"n":2}),
            json!({"waitCompletion":false,"wait_audio":true}),
            json!({"model":null}),
        ] {
            let mut body = json!({"prompt":"synthetic fixture"});
            body.as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            let request = suno::normalize_music_generations(body).unwrap();
            assert_eq!(
                prepare(&request, "chirp-v3-5").unwrap_err().code.as_deref(),
                Some("suno_browser_request_mismatch")
            );
        }
        let mut request =
            suno::normalize_music_generations(json!({"model":"alias","prompt":"fixture"})).unwrap();
        request.raw_body["model"] = json!("conflicting-alias");
        assert!(prepare(&request, "chirp-v3-5").is_err());
        assert!(worker_model(&json!({"model":"chirp-v3-5"})).is_err());
        assert!(worker_model(&json!({})).is_err());
    }
}
