//! Disabled Web and Canvas builds expose explicit refusal, never media/upload transports.
#![cfg(not(any(
    feature = "line-gemini-web-reverse",
    feature = "line-gemini-canvas-program"
)))]
use neuro_gateway::{
    protocol::openai::normalize_chat_completions, routing::candidate::ProviderAccountPayload,
    upstream::gemini::web_reverse as web,
};
use serde_json::json;
use std::time::Duration;

#[tokio::test]
async fn disabled_gemini_execution_entrypoints_refuse_without_network() {
    let trap = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = rquest::Client::builder()
        .no_proxy()
        .proxy(rquest::Proxy::all(format!("http://{}", trap.local_addr().unwrap())).unwrap())
        .timeout(Duration::from_millis(200))
        .build()
        .unwrap();
    let payload: ProviderAccountPayload = serde_json::from_value(json!({
        "adapter":"gemini_web_compatible","base_url":"https://gemini.google.com","api_key":"synthetic-unused"
    })).unwrap();
    let request = normalize_chat_completions(
        json!({"model":"test","messages":[{"role":"user","content":"hello"}]}),
    )
    .unwrap();
    let errors = [
        web::execute(
            &client,
            Duration::from_secs(1),
            &payload,
            &request,
            "test",
            None,
        )
        .await
        .err(),
        web::execute_stream(
            &client,
            Duration::from_secs(1),
            &payload,
            &request,
            "test",
            None,
        )
        .await
        .err(),
        web::execute_speech(
            &client,
            Duration::from_secs(1),
            &payload,
            &request,
            "test",
            None,
        )
        .await
        .err(),
        web::execute_transcription(
            &client,
            Duration::from_secs(1),
            &payload,
            &request,
            "test",
            None,
        )
        .await
        .err(),
    ];
    for error in errors {
        assert_eq!(
            error.unwrap().code.as_deref(),
            Some("gateway_provider_line_compiled_out")
        );
    }
    assert!(
        tokio::time::timeout(Duration::from_millis(30), trap.accept())
            .await
            .is_err()
    );
}
