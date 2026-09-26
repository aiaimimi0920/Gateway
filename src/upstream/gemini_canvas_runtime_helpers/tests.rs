use super::persistence::build_gemini_canvas_runtime_persistence_payload;
use super::session::gemini_canvas_explicit_cookie_header_from_payload;
use super::*;
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::protocol::gemini_canvas;
use crate::routing::candidate::ProviderAccountPayload;
use std::time::{SystemTime, UNIX_EPOCH};

fn make_payload(adapter: &str, base_url: &str) -> ProviderAccountPayload {
    serde_json::from_value(json!({
        "adapter": adapter,
        "baseUrl": base_url,
        "apiKey": "sk-test"
    }))
    .expect("payload")
}

#[test]
fn runtime_api_missing_google_api_key_error_matches_contract() {
    let probes = vec![
        "page1 len=10 contains_aiza=false api_key_count=0".to_string(),
        "page2 fetch_error=timeout".to_string(),
    ];
    let error = gemini_canvas_runtime_api_missing_google_api_key_error(&probes);
    assert_eq!(error.http_status, Some(401));
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    assert_eq!(
        error.code.as_deref(),
        Some("gemini_canvas_runtime_api_missing_google_api_key")
    );
    assert_eq!(
        error.message.as_str(),
        "Gemini Canvas runtime API media lane requires a Google API key harvested from the Canvas session state.; page_harvest=page1 len=10 contains_aiza=false api_key_count=0 | page2 fetch_error=timeout"
    );
}

#[test]
fn origin_from_url_normalizes_scheme_and_strips_path() {
    assert_eq!(
        origin_from_url(" gemini.google.com/app/4abc4e7577b6149f?hl=zh-CN "),
        Some("https://gemini.google.com".to_string())
    );
    assert_eq!(
        origin_from_url("http://example.com/nested/path"),
        Some("http://example.com".to_string())
    );
    assert_eq!(origin_from_url("   "), None);
}

#[test]
fn gemini_canvas_explicit_cookie_header_from_payload_prefers_contract_then_headers_then_storage() {
    let mut payload = make_payload(
        "gemini_canvas_program_web_reverse_compatible",
        "https://gemini.google.com",
    );
    payload.headers.insert(
        "cookie".to_string(),
        "__Secure-1PAPISID=from-header".to_string(),
    );
    payload.extra_body = Some(HashMap::from([(
        "canvasProgramInvokeContract".to_string(),
        json!({
            "cookieHeader": "__Secure-1PAPISID=from-contract; SID=contract"
        }),
    )]));
    let storage_state = json!({
        "cookieHeader": "__Secure-1PAPISID=from-storage"
    });

    assert_eq!(
        gemini_canvas_explicit_cookie_header_from_payload(&payload, Some(&storage_state))
            .as_deref(),
        Some("__Secure-1PAPISID=from-contract; SID=contract")
    );

    payload.extra_body = None;
    assert_eq!(
        gemini_canvas_explicit_cookie_header_from_payload(&payload, Some(&storage_state))
            .as_deref(),
        Some("__Secure-1PAPISID=from-header")
    );

    payload.headers.clear();
    assert_eq!(
        gemini_canvas_explicit_cookie_header_from_payload(&payload, Some(&storage_state))
            .as_deref(),
        Some("__Secure-1PAPISID=from-storage")
    );
}

#[test]
fn gemini_canvas_pure_http_session_from_payload_or_storage_prefers_explicit_cookie_header() {
    let mut payload = make_payload(
        "gemini_canvas_program_web_reverse_compatible",
        "https://gemini.google.com",
    );
    payload.extra_body = Some(HashMap::from([(
        "cookieHeader".to_string(),
        json!("__Secure-1PAPISID=session-token; SID=abc"),
    )]));

    let session = gemini_canvas_pure_http_session_from_payload_or_storage(
        &payload,
        &json!({}),
        "https://gemini.google.com/app/4abc4e7577b6149f",
        "https://gemini.google.com",
        "0",
    )
    .expect("explicit cookie header session");

    assert_eq!(
        session,
        gemini_canvas::GeminiCanvasPureHttpSession {
            cookie_header: "__Secure-1PAPISID=session-token; SID=abc".to_string(),
            sapisid: "session-token".to_string(),
            auth_user: "0".to_string(),
        }
    );
}

#[test]
fn current_unix_timestamp_i64_stays_within_wall_clock_bounds() {
    let before = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let observed = current_unix_timestamp_i64();
    let after = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    assert!(observed >= before);
    assert!(observed <= after);
}

#[test]
fn gemini_canvas_signaler_zx_token_is_32_hex_chars() {
    let token = gemini_canvas_signaler_zx_token();

    assert_eq!(token.len(), 32);
    assert!(token.chars().all(|ch| ch.is_ascii_hexdigit()));
}

#[test]
fn gemini_canvas_runtime_persistence_payload_merges_material_patch() {
    let mut payload = make_payload("gemini_canvas_program_web_reverse_compatible", "   ");
    payload.runtime_state_object_key =
        Some("credential-runtime/gemini-canvas/program/storage-state.json".to_string());
    let existing_payload = json!({
        "extraBody": {
            "existing": "keep",
            "shareId": "old-share"
        },
        "adapter": "   ",
        "base_url": "   "
    });
    let patch = HashMap::from([
        ("shareId".to_string(), json!("new-share")),
        ("appPath".to_string(), json!("/app/4abc4e7577b6149f")),
    ]);

    let updated = build_gemini_canvas_runtime_persistence_payload(
        &existing_payload,
        &payload,
        &patch,
        Some("https://gemini.google.com"),
    );
    let updated_map = updated.as_object().expect("updated payload object");
    let extra_body = updated_map
        .get("extraBody")
        .and_then(Value::as_object)
        .expect("merged extraBody object");

    assert_eq!(extra_body.get("existing"), Some(&json!("keep")));
    assert_eq!(extra_body.get("shareId"), Some(&json!("new-share")));
    assert_eq!(
        extra_body.get("appPath"),
        Some(&json!("/app/4abc4e7577b6149f"))
    );
    assert_eq!(
        updated_map.get("runtimeStateObjectKey"),
        Some(&json!(
            "credential-runtime/gemini-canvas/program/storage-state.json"
        ))
    );
    assert_eq!(
        updated_map.get("adapter"),
        Some(&json!("gemini_canvas_program_web_reverse_compatible"))
    );
    assert_eq!(
        updated_map.get("baseUrl"),
        Some(&json!("https://gemini.google.com"))
    );
}

#[test]
fn gemini_canvas_runtime_persistence_payload_preserves_existing_identity_fields() {
    let mut payload = make_payload(
        "gemini_canvas_program_web_reverse_compatible",
        "https://candidate.example",
    );
    payload.runtime_state_object_key = Some("runtime-state/new.json".to_string());
    let existing_payload = json!({
        "extraBody": {},
        "adapter": "existing_adapter",
        "baseUrl": "https://existing.example"
    });
    let patch = HashMap::from([("shareId".to_string(), json!("new-share"))]);

    let updated = build_gemini_canvas_runtime_persistence_payload(
        &existing_payload,
        &payload,
        &patch,
        Some("https://provider-account.example"),
    );
    let updated_map = updated.as_object().expect("updated payload object");
    let extra_body = updated_map
        .get("extraBody")
        .and_then(Value::as_object)
        .expect("merged extraBody object");

    assert_eq!(extra_body.get("shareId"), Some(&json!("new-share")));
    assert_eq!(updated_map.get("adapter"), Some(&json!("existing_adapter")));
    assert_eq!(
        updated_map.get("baseUrl"),
        Some(&json!("https://existing.example"))
    );
    assert_eq!(
        updated_map.get("runtimeStateObjectKey"),
        Some(&json!("runtime-state/new.json"))
    );
}

#[tokio::test]
async fn persist_gemini_canvas_program_runtime_material_noops_without_required_inputs() {
    let mut payload = make_payload(
        "gemini_canvas_program_web_reverse_compatible",
        "https://gemini.google.com",
    );

    persist_gemini_canvas_program_runtime_material(
        None,
        None,
        &payload,
        Some(HashMap::from([("shareId".to_string(), json!("new-share"))])),
    )
    .await;

    payload.credential_id = Some("credential-123".to_string());
    persist_gemini_canvas_program_runtime_material(None, None, &payload, None).await;
}
