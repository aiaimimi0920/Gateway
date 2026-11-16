//! Generic passthrough/discovery remain closed; inference requires the dedicated catalog-bound lane.
use crate::{error::GatewayError, routing::candidate::ProviderAccountPayload};
pub const BASE: &str = "https://lobsterai-server.youdao.com/api/proxy/v1";
pub fn reject_generic(payload: &ProviderAccountPayload) -> Result<(), GatewayError> {
    if payload.auth_mode.as_deref() == Some(super::AUTH_MODE)
        || payload.base_url.trim_end_matches('/') == BASE
    {
        return Err(GatewayError::bad_request(
            "LobsterAI requires the dedicated catalog-bound inference route",
        )
        .with_code("lobsterai_inference_unavailable"));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn lobsterai_management_stage_blocks_real_plan_buffered_streaming_and_native_entrypoints()
    {
        use crate::routing::candidate::ProviderExecutionMode;
        use crate::upstream::client::UpstreamClient;
        // A regression still has only a loopback target, never a provider host or real credential.
        let payload = serde_json::from_value(serde_json::json!({
            "adapter":"openai_compatible", "auth_mode":super::super::AUTH_MODE,
            "base_url":"http://127.0.0.1:9/v1", "api_key":"synthetic-access"
        }))
        .unwrap();
        let request = crate::protocol::openai::normalize_chat_completions(serde_json::json!({
            "model":"opaque", "messages":[{"role":"user","content":"synthetic"}]
        }))
        .unwrap();
        let client = UpstreamClient::new(1);
        let mut errors =
            vec![
                UpstreamClient::build_request_plan(&payload, &request, "opaque", false)
                    .err()
                    .expect("blocked plan"),
            ];
        errors.push(
            client
                .execute_with_provider_account_id("synthetic", &payload, &request, "opaque", None)
                .await
                .err()
                .expect("blocked buffered"),
        );
        errors.push(
            client
                .execute_stream(&payload, &request, "opaque", None)
                .await
                .err()
                .expect("blocked streaming"),
        );
        errors.push(
            client
                .execute_json_passthrough(
                    "synthetic",
                    ProviderExecutionMode::DirectHttp,
                    &payload,
                    &request,
                    "opaque",
                    None,
                )
                .await
                .err()
                .expect("blocked passthrough"),
        );
        let responses = crate::protocol::responses::normalize_responses(
            serde_json::json!({"model":"opaque","input":"synthetic"}),
        )
        .unwrap();
        errors.push(
            client
                .execute_stream_with_wire(&payload, &responses, "opaque", None, None)
                .await
                .err()
                .expect("blocked native streaming"),
        );
        errors.push(
            client
                .execute_json_with_wire(
                    "synthetic",
                    ProviderExecutionMode::DirectHttp,
                    &payload,
                    &responses,
                    "opaque",
                    None,
                    None,
                )
                .await
                .err()
                .expect("blocked native buffered"),
        );
        for error in errors {
            assert_eq!(
                error.code.as_deref(),
                Some("lobsterai_inference_unavailable")
            );
        }
    }
    #[test]
    fn lobsterai_management_does_not_admit_generic_inference_or_discovery() {
        for value in [
            serde_json::json!({"adapter":"openai_compatible", "base_url":"http://127.0.0.1:9", "api_key":"", "auth_mode":super::super::AUTH_MODE}),
            serde_json::json!({"adapter":"openai_compatible", "api_key":"", "base_url":BASE}),
        ] {
            let payload = serde_json::from_value(value).unwrap();
            assert_eq!(
                reject_generic(&payload).unwrap_err().code.as_deref(),
                Some("lobsterai_inference_unavailable")
            );
        }
        assert!(reject_generic(
            &serde_json::from_value(serde_json::json!({"adapter":"openai_compatible", "api_key":"", "base_url":"https://example.invalid/v1"}))
                .unwrap()
        )
        .is_ok());
    }
}
