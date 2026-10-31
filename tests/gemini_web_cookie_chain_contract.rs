//! A bootstrap Cookie update must reach only the same request's subsequent HTTP phase.
#[path = "gemini_web_cookie_chain_contract/fixture.rs"]
mod fixture;
#[path = "gemini_web_speech_contract/fixture.rs"]
mod speech_fixture;

use axum::http::StatusCode;
use fixture::{fixture, Options};
use neuro_gateway::{
    protocol::openai::{normalize_audio_speech, normalize_chat_completions},
    routing::candidate::ProviderAccountPayload,
    upstream::{client::UpstreamClient, gemini::web_reverse},
};
use serde_json::json;
use std::time::Duration;

fn payload(base: &str) -> ProviderAccountPayload {
    serde_json::from_value(json!({"adapter":"gemini_web_compatible", "base_url":base,
        "api_key":"synthetic-seed", "chat_completions_path":"/generate",
        "headers":{"cookie":"pref=selected"}, "extra_body":{"pureHttpMode":"required"}}))
    .unwrap()
}

async fn text(base: &str) -> Result<(), neuro_gateway::error::GatewayError> {
    let http = neuro_gateway::http_client::builder()
        .no_proxy()
        .build()
        .unwrap();
    let request = normalize_chat_completions(json!({"model":"gemini-web-chat",
        "messages":[{"role":"user","content":"synthetic"}]}))
    .unwrap();
    web_reverse::execute(
        &http,
        Duration::from_secs(2),
        &payload(base),
        &request,
        "gemini-web-chat",
        None,
    )
    .await
    .map(|_| ())
}

#[tokio::test]
async fn cookie_chain_text_uses_rotated_bootstrap_cookie() {
    let f = fixture(Options {
        cookies: vec!["pref=rotated; Path=/"],
        ..Options::default()
    })
    .await;
    text(&f.base).await.unwrap();
    let state = f.capture.lock().unwrap();
    assert_eq!(state.gets, 1);
    assert_eq!(state.cookies.len(), 1);
    assert!(
        state.cookies[0].contains("pref=rotated"),
        "{}",
        state.cookies[0]
    );
    assert!(!state.cookies[0].contains("pref=selected"));
}

#[tokio::test]
async fn cookie_chain_speech_keeps_seed_but_sends_rotated_cookie() {
    let f = fixture(Options {
        cookies: vec!["pref=rotated; Path=/"],
        speech: true,
        ..Options::default()
    })
    .await;
    let seed = payload(&f.base);
    let before = serde_json::to_value(&seed).unwrap();
    let req = normalize_audio_speech(
        json!({"model":"gemini-web-chat", "input":"synthetic", "response_format":"wav"}),
    )
    .unwrap();
    let reply = UpstreamClient::new(2)
        .execute_binary_passthrough(&seed, &req, "gemini-web-chat", None)
        .await
        .unwrap();
    assert_eq!(reply.body.as_ref(), speech_fixture::wav());
    assert_eq!(serde_json::to_value(seed).unwrap(), before);
    let state = f.capture.lock().unwrap();
    assert_eq!(state.gets, 1);
    assert_eq!(state.cookies.len(), 1);
    assert!(
        state.cookies[0].contains("pref=rotated"),
        "{}",
        state.cookies[0]
    );
}

#[tokio::test]
async fn cookie_chain_deleted_seed_is_not_resurrected() {
    let f = fixture(Options {
        cookies: vec!["pref=; Path=/; Max-Age=0"],
        ..Options::default()
    })
    .await;
    text(&f.base).await.unwrap();
    assert!(!f.capture.lock().unwrap().cookies[0].contains("pref="));
}

#[tokio::test]
async fn cookie_chain_bootstrap_redirect_never_follows_or_generates() {
    let f = fixture(Options {
        status: StatusCode::FOUND,
        ..Options::default()
    })
    .await;
    assert!(text(&f.base).await.is_err());
    let state = f.capture.lock().unwrap();
    assert_eq!(state.targets, 0);
    assert!(state.cookies.is_empty());
}

#[tokio::test]
async fn cookie_chain_bootstrap_body_limit_precedes_generation() {
    for chunked in [false, true] {
        let f = fixture(Options {
            app: format!("{}{}", fixture::APP, "x".repeat(8 * 1024 * 1024)),
            chunked,
            ..Options::default()
        })
        .await;
        assert!(text(&f.base).await.is_err());
        assert!(f.capture.lock().unwrap().cookies.is_empty());
    }
}

#[tokio::test]
async fn cookie_chain_streaming_keeps_the_same_rotated_session() {
    use futures::StreamExt;
    let f = fixture(Options {
        cookies: vec!["pref=rotated; Path=/"],
        ..Options::default()
    })
    .await;
    let http = neuro_gateway::http_client::builder()
        .no_proxy()
        .build()
        .unwrap();
    let req = normalize_chat_completions(json!({"model":"gemini-web-chat", "stream":true,
        "messages":[{"role":"user","content":"synthetic"}]}))
    .unwrap();
    let mut stream = web_reverse::execute_stream(
        &http,
        Duration::from_secs(2),
        &payload(&f.base),
        &req,
        "gemini-web-chat",
        None,
    )
    .await
    .unwrap();
    let mut bytes = vec![];
    while let Some(chunk) = stream.next().await {
        bytes.extend_from_slice(&chunk.unwrap());
    }
    assert!(String::from_utf8(bytes).unwrap().contains("synthetic"));
    let state = f.capture.lock().unwrap();
    assert_eq!(state.cookies.len(), 1);
    assert!(state.cookies[0].contains("pref=rotated"));
}

#[tokio::test]
async fn cookie_chain_shared_client_starts_each_invocation_from_its_own_seed() {
    let f = fixture(Options {
        cookies: vec!["pref=rotated; Path=/"],
        ..Options::default()
    })
    .await;
    let http = neuro_gateway::http_client::builder()
        .no_proxy()
        .build()
        .unwrap();
    let req = normalize_chat_completions(json!({"model":"gemini-web-chat",
        "messages":[{"role":"user","content":"synthetic"}]}))
    .unwrap();
    for _ in 0..2 {
        web_reverse::execute(
            &http,
            Duration::from_secs(2),
            &payload(&f.base),
            &req,
            "gemini-web-chat",
            None,
        )
        .await
        .unwrap();
    }
    let state = f.capture.lock().unwrap();
    assert_eq!(state.gets, 2);
    assert!(state
        .initial_cookies
        .iter()
        .all(|cookie| cookie.contains("pref=selected")));
    assert_eq!(state.cookies.len(), 2);
    assert!(state
        .cookies
        .iter()
        .all(|cookie| cookie.contains("pref=rotated")));
}
