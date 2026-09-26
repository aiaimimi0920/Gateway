use super::super::{bootstrap_site, execute, execute_stream, get_requirements, post_prepare};
use super::{make_payload, make_request};

use crate::error::GatewayError;
use crate::protocol::chatgpt::web_reverse as surface;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::chatgpt::web_reverse::build_request_context;

#[derive(Clone, Copy, Debug)]
pub(super) enum Target {
    Bootstrap,
    Requirements,
    Prepare,
    Conversation,
    StreamFailure,
    StreamNonSse,
}

impl Target {
    pub(super) fn path(self) -> &'static str {
        match self {
            Self::Bootstrap => "/",
            Self::Requirements => surface::CHATGPT_WEB_DEFAULT_REQUIREMENTS_PATH,
            Self::Prepare => surface::CHATGPT_WEB_DEFAULT_F_CONVERSATION_PREPARE_PATH,
            _ => surface::CHATGPT_WEB_DEFAULT_F_CONVERSATION_PATH,
        }
    }
    pub(super) fn status(self) -> u16 {
        if matches!(self, Self::StreamFailure) {
            503
        } else {
            200
        }
    }
    pub(super) fn content_type(self) -> &'static str {
        match self {
            Self::Bootstrap => "text/html",
            Self::Conversation => "text/event-stream",
            _ => "application/json",
        }
    }
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Bootstrap => "ChatGPT Web reverse bootstrap body",
            Self::Requirements => "ChatGPT Web reverse requirements body",
            Self::Prepare => "ChatGPT Web reverse prepare body",
            _ => "ChatGPT Web reverse conversation body",
        }
    }
}

pub(super) fn cached_payload(url: &str) -> ProviderAccountPayload {
    let mut payload = make_payload(url);
    payload.extra_body.as_mut().unwrap().insert(
        "chatgptWebSentinelChatRequirementsToken".to_string(),
        serde_json::json!("fixture-token"),
    );
    payload
}

pub(super) async fn invoke(target: Target, url: &str) -> Result<String, GatewayError> {
    let http = rquest::Client::builder()
        .no_proxy()
        .build()
        .expect("client");
    let timeout = std::time::Duration::from_secs(5);
    let mut payload = cached_payload(url);
    let context = build_request_context(&payload);
    match target {
        Target::Bootstrap => {
            payload.extra_body = None;
            bootstrap_site(&http, timeout, &payload, &context, None)
                .await
                .map(|result| result.pow_data_build.unwrap_or_default())
        }
        Target::Requirements => {
            let bootstrap = surface::bootstrap_from_payload_cache(payload.extra_body.as_ref())
                .expect("cached bootstrap");
            get_requirements(&http, timeout, &payload, &context, &bootstrap, None)
                .await
                .map(|result| result.token)
        }
        Target::Prepare => post_prepare(
            &http,
            timeout,
            &payload,
            &context,
            None,
            target.path(),
            "fixture-trace",
        )
        .await
        .map(|()| String::new()),
        Target::Conversation => execute(&http, timeout, &payload, &make_request(), "auto", None)
            .await
            .map(|result| result.text),
        Target::StreamFailure | Target::StreamNonSse => {
            let mut request = make_request();
            request.stream = true;
            execute_stream(&http, timeout, &payload, &request, "auto", None)
                .await
                .map(|_| String::new())
        }
    }
}
