//! Only the fixture transport destination changes; use the registered preset and real sender.
use super::*;
use axum::{
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Json, Router,
};

const MODEL: &str = "vendor/Case-Sensitive-Chat";
const KEY: &str = "zenmux-synthetic-contract";

async fn reply(headers: HeaderMap, Json(body): Json<Value>) -> axum::response::Response {
    assert_eq!(headers["authorization"], format!("Bearer {KEY}").as_str());
    assert!(!headers.contains_key("cookie"));
    assert_eq!(body["model"], MODEL);
    assert!(!body.to_string().contains(KEY));
    if body["messages"][0]["content"] == "deny" {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":{"message":"invalid credential"}})),
        )
            .into_response();
    }
    if body["stream"] == true {
        return (
            [("content-type", "text/event-stream")],
            "data: {\"choices\":[{\"delta\":{\"content\":\"OK\"}}]}\n\ndata: [DONE]\n\n",
        )
            .into_response();
    }
    if body["messages"].as_array().unwrap().len() > 1 {
        assert_eq!(body["messages"][1]["tool_calls"][0]["id"], "call_1");
        assert_eq!(body["messages"][2]["tool_call_id"], "call_1");
        assert_eq!(body["messages"][2]["content"], "42");
        return Json(json!({"model":MODEL,"choices":[{"message":{"role":"assistant","content":"42"},"finish_reason":"stop"}]})).into_response();
    }
    assert_eq!(body["tools"][0]["function"]["name"], "lookup");
    Json(json!({"model":MODEL,"choices":[{"message":{"role":"assistant","tool_calls":[
        {"id":"call_1","type":"function","function":{"name":"lookup","arguments":"{}"}}]},"finish_reason":"tool_calls"}]})).into_response()
}

#[tokio::test]
async fn zenmux_chat_json_tools_sse_and_upstream_error() {
    // The default tokio test runtime is single-threaded. Capture the actual sender's events.
    let output = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
    #[derive(Clone)]
    struct Writer(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
    impl std::io::Write for Writer {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let writer = Writer(output.clone());
    let _guard = tracing::subscriber::set_default(
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_writer(move || writer.clone())
            .finish(),
    );
    tracing::debug!("zenmux capture active");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/api/v1/", listener.local_addr().unwrap());
    let app = Router::new().route("/api/v1/chat/completions", post(reply));
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let preset = crate::preset::get_builtin_preset("zenmux-api").unwrap();
    assert!(
        preset.default_model.is_none()
            && preset.responses_path.is_none()
            && preset.balance_path.is_none()
    );
    let overrides =
        serde_json::from_value(json!({"base_url":crate::protocol::zenmux::BASE,"api_key":KEY}))
            .unwrap();
    let mut account = crate::preset::compile_provider_account(&preset, &overrides);
    crate::protocol::zenmux::validate(Some("zenmux-api"), &account).unwrap();
    assert_eq!(
        crate::routing::protocol_resolution::payload_supported_wire_protocol_families(
            &account,
            "openai_chat",
            Some(MODEL)
        ),
        [crate::protocol::registry::OPENAI_CHAT_FAMILY]
    );
    account.base_url = base;
    assert!(crate::protocol::zenmux::validate(Some("zenmux-api"), &account).is_err());
    account.call_flow = Some(
        serde_json::from_value(
            json!({"call_adapter":"auto","fingerprint_adapter":"auto","special_adapter":"auto"}),
        )
        .unwrap(),
    );
    let mut client = UpstreamClient::new(3);
    client.http = rquest::Client::builder().no_proxy().build().unwrap();
    let user = json!({"role":"user","content":"lookup"});
    let req = openai::normalize_chat_completions(json!({"model":MODEL,"messages":[user],"tools":[
        {"type":"function","function":{"name":"lookup","parameters":{"type":"object","properties":{}}}}]})).unwrap();
    let answer = client.execute(&account, &req, MODEL, None).await.unwrap();
    let tool = &answer.tool_calls[0];
    assert_eq!(tool.name.as_deref(), Some("lookup"));
    assert_eq!(tool.arguments.as_deref(), Some("{}"));
    let req = openai::normalize_chat_completions(json!({"model":MODEL,"messages":[user,
        {"role":"assistant","tool_calls":[{"id":tool.id,"type":"function","function":{"name":tool.name,"arguments":tool.arguments}}]},
        {"role":"tool","tool_call_id":tool.id,"content":"42"}]})).unwrap();
    assert_eq!(
        client
            .execute(&account, &req, MODEL, None)
            .await
            .unwrap()
            .text,
        "42"
    );
    let req =
        openai::normalize_chat_completions(json!({"model":MODEL,"messages":[user],"stream":true}))
            .unwrap();
    let UpstreamStreamingResponse::Http(response) = client
        .execute_stream(&account, &req, MODEL, None)
        .await
        .unwrap()
    else {
        panic!("expected SSE")
    };
    let text = response.text().await.unwrap();
    assert!(text.contains("\"content\":\"OK\"") && text.contains("data: [DONE]"));
    let req = openai::normalize_chat_completions(
        json!({"model":MODEL,"messages":[{"role":"user","content":"deny"}]}),
    )
    .unwrap();
    let err = client
        .execute(&account, &req, MODEL, None)
        .await
        .unwrap_err();
    assert_eq!(err.http_status, Some(401));
    assert!(!err.to_string().contains(KEY));
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
    let trace = String::from_utf8(output.lock().unwrap().clone()).unwrap();
    assert!(trace.contains("zenmux capture active") && !trace.contains(KEY));
}
