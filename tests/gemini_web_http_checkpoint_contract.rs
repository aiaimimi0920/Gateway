//! Explicit opt-in HTTP state survives fresh processes without opening any browser.
#[path = "gemini_web_http_checkpoint_contract/child.rs"]
mod child;
#[path = "gemini_web_http_checkpoint_contract/fixture.rs"]
mod fixture;
#[path = "gemini_web_speech_contract/fixture.rs"]
mod speech_fixture;
use neuro_gateway::{
    protocol::openai::{normalize_audio_speech, normalize_chat_completions},
    routing::candidate::ProviderAccountPayload,
    upstream::gemini::web_reverse,
};
use serde_json::json;
use std::time::Duration;

#[tokio::test]
async fn checkpoint_child() {
    child::run().await;
}

#[tokio::test]
async fn http_checkpoint_survives_fresh_processes_for_text_stream_and_speech() {
    for mode in ["text", "stream", "speech"] {
        let f = fixture::fixture().await;
        let dir = child::directory();
        child::invoke(&f.base, &dir, mode, "selected", "").await;
        f.capture.lock().unwrap().expected = Some("pref=post".into());
        child::invoke(&f.base, &dir, mode, "selected", "").await;
        let state = f.capture.lock().unwrap();
        assert_eq!(state.gets.len(), 2);
        assert_eq!(state.posts.len(), 2);
        assert!(state.gets[1].contains("pref=post"));
        assert!(!state.gets[1].contains("pref=selected"));
    }
}

#[tokio::test]
async fn http_checkpoint_auth_failure_blocks_same_seed_without_contacting_upstream() {
    let f = fixture::fixture().await;
    let dir = child::directory();
    child::invoke(&f.base, &dir, "text", "selected", "").await;
    f.capture.lock().unwrap().fail_post = 401;
    child::invoke(
        &f.base,
        &dir,
        "text",
        "selected",
        "gemini_web_session_invalid",
    )
    .await;
    child::invoke(
        &f.base,
        &dir,
        "text",
        "selected",
        "gemini_web_http_auth_checkpoint_session_invalid",
    )
    .await;
    assert_eq!(f.capture.lock().unwrap().gets.len(), 2);
    assert_eq!(f.capture.lock().unwrap().posts.len(), 2);
    f.capture.lock().unwrap().fail_post = 0;
    child::invoke(&f.base, &dir, "text", "fresh-import", "").await;
    assert!(f.capture.lock().unwrap().gets[2].contains("pref=fresh-import"));
}

#[tokio::test]
async fn http_checkpoint_normal_failure_keeps_previous_committed_state() {
    let f = fixture::fixture().await;
    let dir = child::directory();
    child::invoke(&f.base, &dir, "text", "selected", "").await;
    {
        let mut state = f.capture.lock().unwrap();
        state.expected = Some("pref=post".into());
        state.fail_post = 503;
    }
    child::invoke(&f.base, &dir, "text", "selected", "*").await;
    f.capture.lock().unwrap().fail_post = 0;
    child::invoke(&f.base, &dir, "text", "selected", "").await;
    assert_eq!(f.capture.lock().unwrap().posts.len(), 3);
}

#[tokio::test]
async fn http_checkpoint_deletion_survives_restart_without_seed_resurrection() {
    let f = fixture::fixture().await;
    let dir = child::directory();
    f.capture.lock().unwrap().delete = true;
    for _ in 0..2 {
        child::invoke(&f.base, &dir, "text", "selected", "").await;
    }
    let state = f.capture.lock().unwrap();
    assert!(state.gets[0].contains("pref=selected"));
    assert!(!state.gets[1].contains("pref="));
}

#[tokio::test]
async fn http_checkpoint_save_failure_preserves_success_without_replay() {
    let f = fixture::fixture().await;
    let dir = child::directory();
    f.capture.lock().unwrap().break_storage = Some(dir.clone());
    child::invoke(&f.base, &dir, "text", "selected", "").await;
    let state = f.capture.lock().unwrap();
    assert_eq!(state.gets.len(), 1);
    assert_eq!(state.posts.len(), 1);
    assert!(dir.join("preserved-checkpoint.sqlite3").is_file());
}

#[tokio::test]
async fn http_checkpoint_invalid_configuration_stops_before_network_and_disabled_keeps_legacy() {
    let f = fixture::fixture().await;
    let dir = child::directory();
    child::invoke(
        &f.base,
        &dir,
        "invalid-config",
        "selected",
        "gemini_web_http_auth_checkpoint_config_invalid",
    )
    .await;
    assert!(f.capture.lock().unwrap().gets.is_empty());
    for _ in 0..2 {
        child::invoke(&f.base, &dir, "disabled", "selected", "").await;
    }
    assert!(f
        .capture
        .lock()
        .unwrap()
        .gets
        .iter()
        .all(|value| value.contains("pref=selected")));
    assert!(!dir.join("http-auth-checkpoints-v1.sqlite3").exists());
}

#[tokio::test]
async fn http_checkpoint_invalid_response_does_not_persist_bootstrap_or_post_cookie() {
    let f = fixture::fixture().await;
    let dir = child::directory();
    f.capture.lock().unwrap().invalid_json = true;
    child::invoke(&f.base, &dir, "text", "selected", "*").await;
    f.capture.lock().unwrap().invalid_json = false;
    child::invoke(&f.base, &dir, "text", "selected", "").await;
    assert!(f.capture.lock().unwrap().gets[1].contains("pref=selected"));
}
