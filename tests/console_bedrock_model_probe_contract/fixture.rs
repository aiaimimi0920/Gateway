//! Synthetic Bedrock wire only. Legacy loopback targets never weaken explicit AWS binding.
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Method, StatusCode, Uri},
    Router,
};
use bytes::Bytes;
use neuro_gateway::routing::config::RouteConfigYaml;
use serde_json::{json, Value};
use std::{
    convert::Infallible,
    sync::{Arc, Mutex},
};
use tokio::task::JoinHandle;

#[path = "../bedrock_upstream_eventstream_contract/fixtures.rs"]
pub mod wire;

pub const MODEL: &str = "arn:aws:bedrock:us-east-1:123456789012:inference-profile/us.fixture-v1:0";

pub struct Upstream {
    pub base: String,
    pub calls: Arc<Mutex<Vec<String>>>,
    task: JoinHandle<()>,
}

impl Upstream {
    pub async fn start() -> Self {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let app = Router::new().fallback(respond).with_state(calls.clone());
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self { base, calls, task }
    }

    pub fn document(&self, ids: &[&str]) -> RouteConfigYaml {
        serde_json::from_value(json!({"providers":[{
            "id":"bedrock-test", "preset":"bedrock-converse", "base_url":self.base,
            "explicit_credentials_only":true,
            "supported_models":["probe-alias"], "model_map":{"probe-alias":MODEL},
            "extra_body":{"inferenceConfig":{"maxTokens":9999},"stream":false,"modelId":"wrong"},
            "credentials":ids.iter().map(|id|json!({"id":id,"api_key":format!("{id}-secret"),
                "enabled":*id!="disabled", "supported_models":[MODEL]})).collect::<Vec<_>>()
        }],"model_routes":[],"aliases":{}}))
        .unwrap()
    }

    pub async fn finish(self) {
        self.task.abort();
        assert!(self.task.await.unwrap_err().is_cancelled());
    }
}

pub fn sequence(text: &str, reason: &str) -> Vec<Vec<u8>> {
    vec![
        wire::event("messageStart", json!({"role":"assistant"})),
        wire::event(
            "contentBlockDelta",
            json!({"contentBlockIndex":0,"delta":{"text":text}}),
        ),
        wire::event("contentBlockStop", json!({"contentBlockIndex":0})),
        wire::event("messageStop", json!({"stopReason":reason})),
        wire::event(
            "metadata",
            json!({"usage":{"inputTokens":5,"outputTokens":2,"totalTokens":7}}),
        ),
    ]
}

async fn respond(
    State(calls): State<Arc<Mutex<Vec<String>>>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    axum::Json(body): axum::Json<Value>,
) -> axum::response::Response {
    let authorization = headers["authorization"].to_str().unwrap();
    let key = if !authorization.starts_with("AWS4-HMAC-SHA256 ") {
        // The legacy preset's explicit authorization header preserves a raw token.
        assert!(authorization.ends_with("-secret"));
        authorization.to_owned()
    } else {
        assert!(authorization.starts_with("AWS4-HMAC-SHA256 "));
        assert!(authorization.contains("Credential=synthetic/"));
        assert_eq!(headers["x-amz-security-token"], "synthetic-long-session");
        "iam-secret".into()
    };
    calls.lock().unwrap().push(key.clone());
    assert_eq!(method, Method::POST);
    assert_eq!(
        uri.path(),
        neuro_gateway::protocol::bedrock_converse::default_path(MODEL, true)
    );
    assert!(uri.query().is_none());
    assert_eq!(
        body,
        json!({"messages":[{"role":"user","content":[{"text":"Reply OK"}]}],
        "inferenceConfig":{"maxTokens":32}})
    );
    let id = key.strip_suffix("-secret").unwrap();
    let (status, content_type, bytes) = reply(id);
    let chunks: Vec<_> = bytes
        .chunks(7)
        .map(|chunk| Ok::<_, Infallible>(Bytes::copy_from_slice(chunk)))
        .collect();
    axum::response::Response::builder()
        .status(status)
        .header("content-type", content_type)
        .body(Body::from_stream(futures::stream::iter(chunks)))
        .unwrap()
}

fn reply(id: &str) -> (StatusCode, &'static str, Vec<u8>) {
    let mut frames = sequence(
        if id == "echo" { "OK echo-secret" } else { "OK" },
        "end_turn",
    );
    match id {
        "unauthorized" => {
            return (
                StatusCode::UNAUTHORIZED,
                "application/json",
                b"unauthorized-secret".to_vec(),
            )
        }
        "forbidden" => {
            return (
                StatusCode::FORBIDDEN,
                "application/json",
                b"forbidden-secret".to_vec(),
            )
        }
        "limited" => {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                "application/json",
                b"limited-secret".to_vec(),
            )
        }
        "badcrc" => {
            frames[1][15] ^= 1;
        }
        "badprelude" => {
            frames[0][8] ^= 1;
        }
        "missing-usage" => {
            frames.pop();
        }
        "missing-stop" => {
            frames.truncate(3);
        }
        "truncated" => {
            frames.last_mut().unwrap().pop();
        }
        "extra-tail" => {
            frames.push(wire::event("messageStart", json!({"role":"assistant"})));
        }
        "empty" => frames = sequence(" \n", "end_turn"),
        "length" => frames = sequence("OK", "max_tokens"),
        "filtered" => frames = sequence("OK", "content_filtered"),
        "unknown-stop" => frames = sequence("OK", "unrecognized"),
        "long-text" => frames = sequence(&"x".repeat(8193), "end_turn"),
        "tool-text" => {
            frames = wire::sequence();
            let index = frames.len() - 2;
            frames[index] = wire::event("messageStop", json!({"stopReason":"end_turn"}));
        }
        "duplicate-usage" => frames.push(frames.last().unwrap().clone()),
        "iam" => {
            frames = sequence(
                "OK synthetic synthetic-long-secret synthetic-long-session",
                "end_turn",
            )
        }
        "oversized" => frames = sequence(&"x".repeat(262144), "end_turn"),
        "exception" => frames.push(wire::frame(
            &[
                (":message-type", "exception"),
                (":exception-type", "throttlingException"),
                (":content-type", "application/json"),
            ],
            &json!({"message":"exception-secret"}),
        )),
        "sse" => {
            return (
                StatusCode::OK,
                "text/event-stream",
                b"data: {\"choices\":[{\"delta\":{\"content\":\"OK\"}}]}\n\ndata: [DONE]\n\n"
                    .to_vec(),
            )
        }
        "json" => {
            return (
                StatusCode::OK,
                "application/json",
                b"{\"output\":\"OK\"}".to_vec(),
            )
        }
        "good" | "echo" => {}
        _ => panic!("unexpected synthetic credential"),
    }
    (
        StatusCode::OK,
        "application/vnd.amazon.eventstream",
        frames.concat(),
    )
}
