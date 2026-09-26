use super::{make_payload, make_request, UpstreamClient};
use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayResponse, EndpointKind};

pub(super) const CHAT_PATH: &str = "/api.openai.com/v1/chat/completions";
pub(super) const RESPONSES_PATH: &str = "/api.openai.com/v1/responses";

#[derive(Clone, Copy, Debug)]
pub(super) enum Target {
    Json,
    NonstreamError,
    ForcedError,
    StreamError,
}

impl Target {
    pub(super) fn path(self) -> &'static str {
        if matches!(self, Self::ForcedError) {
            RESPONSES_PATH
        } else {
            CHAT_PATH
        }
    }
    pub(super) fn status(self) -> u16 {
        if matches!(self, Self::Json) {
            200
        } else {
            429
        }
    }
    pub(super) fn label(self) -> &'static str {
        if matches!(self, Self::Json) {
            "ChatGPT official API JSON body"
        } else {
            "ChatGPT official API error body"
        }
    }
    pub(super) async fn invoke(self, url: &str) -> Result<(), GatewayError> {
        let client = client();
        let mut payload = make_payload(&format!("{url}/api.openai.com"));
        let mut request = make_request(EndpointKind::ChatCompletions);
        match self {
            Self::Json | Self::NonstreamError => client
                .execute_chatgpt_official_nonstreaming(&payload, &request, "fixture-model", None)
                .await
                .map(|_| ()),
            Self::ForcedError => {
                payload.responses_path = Some("/v1/responses".to_string());
                client
                    .execute_chatgpt_official_forced_streaming_accumulate(
                        &payload,
                        &request,
                        "fixture-model",
                        None,
                    )
                    .await
                    .map(|_| ())
            }
            Self::StreamError => {
                request.stream = true;
                client
                    .execute_chatgpt_official_streaming(&payload, &request, "fixture-model", None)
                    .await
                    .map(|_| ())
            }
        }
    }
}

pub(super) fn client() -> UpstreamClient {
    let mut client = UpstreamClient::new(30);
    client.http = rquest::Client::builder()
        .no_proxy()
        .build()
        .expect("no-proxy client");
    client
}

pub(super) async fn read_json(
    url: &str,
    endpoint: EndpointKind,
) -> Result<CanonicalRelayResponse, GatewayError> {
    client()
        .execute_chatgpt_official_nonstreaming(
            &make_payload(&format!("{url}/api.openai.com")),
            &make_request(endpoint),
            "fixture-model",
            None,
        )
        .await
}
