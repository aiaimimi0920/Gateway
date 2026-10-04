use crate::error::GatewayError;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
};
use crate::routing::candidate::{ProviderAccountPayload, ProviderExecutionMode};
use crate::upstream::client::UpstreamClient;

#[derive(Clone, Copy, Debug)]
pub(super) enum Target {
    Buffered,
    Stream,
    Forced,
    Json,
    Binary,
    OfficialBuffered,
    OfficialStream,
    OfficialForced,
    Anthropic,
}

impl Target {
    pub(super) async fn invoke(
        self,
        client: &UpstreamClient,
        url: &str,
    ) -> Result<(), GatewayError> {
        let official = matches!(
            self,
            Self::OfficialBuffered | Self::OfficialStream | Self::OfficialForced
        );
        let mut payload: ProviderAccountPayload = serde_json::from_value(serde_json::json!({
            "adapter": if matches!(self, Self::Anthropic) { "anthropic_compatible" } else { "openai_compatible" },
            "base_url": if official { format!("{url}/api.openai.com") } else { url.to_string() },
            "api_key": "synthetic-test-key", "execution_mode": "direct_http"
        })).unwrap();
        if matches!(self, Self::Forced | Self::OfficialForced) {
            payload.responses_path = Some("/v1/responses".to_string());
        }
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: match self {
                Self::Json => EndpointKind::Embeddings,
                Self::Binary => EndpointKind::AudioSpeech,
                _ => EndpointKind::ChatCompletions,
            },
            requested_model: Some("fixture-model".to_string()),
            stream: matches!(self, Self::Stream | Self::OfficialStream),
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "fixture".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: Vec::new(),
            }],
            tools: Vec::new(),
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: serde_json::json!({"input":"fixture", "voice":"fixture"}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: Default::default(),
        };
        match self {
            Self::Stream | Self::OfficialStream => client
                .execute_stream(&payload, &req, "fixture-model", None)
                .await
                .map(|_| ()),
            Self::Json => client
                .execute_json_passthrough(
                    "fixture",
                    ProviderExecutionMode::DirectHttp,
                    &payload,
                    &req,
                    "fixture-model",
                    None,
                )
                .await
                .map(|_| ()),
            Self::Binary => client
                .execute_binary_passthrough(&payload, &req, "fixture-model", None)
                .await
                .map(|_| ()),
            _ => client
                .execute_with_provider_account_id("fixture", &payload, &req, "fixture-model", None)
                .await
                .map(|_| ()),
        }
    }
}

pub(super) fn client() -> UpstreamClient {
    let mut client = UpstreamClient::new(5);
    client.http = rquest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap();
    client
}
