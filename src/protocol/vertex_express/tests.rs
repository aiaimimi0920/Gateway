use super::*;
use crate::routing::config::{RouteConfigStore, RouteConfigYaml};
use serde_json::{json, Value};

fn provider() -> Value {
    json!({"id":"renamed-product","preset":PRESET_ID,"base_url":BASE_URL,
        "explicit_credentials_only":true,"credentials":[{"id":"account","api_key":"synthetic-express-key","supported_models":["gemini-contract"]}]})
}
fn compile(value: Value) -> anyhow::Result<RouteConfigStore> {
    RouteConfigStore::from_document(serde_json::from_value(json!({"providers":[value]})).unwrap())
}
#[cfg(feature = "line-google-agent-platform-official")]
fn payload() -> ProviderAccountPayload {
    compile(provider()).unwrap().snapshot().get_providers()[0].credential_pool[0]
        .payload
        .clone()
}
#[test]
fn vertex_express_opaque_material_and_resource_segment_are_bounded() {
    assert!(valid_key("not-a-guessed-prefix"));
    for value in ["", "two words", "secret\r\nheader", "中文"] {
        assert!(!valid_key(value));
    }
    assert!(!valid_key(&"k".repeat(32_769)));
    assert!(valid_model("gemini-2.5-flash-001"));
    for model in [
        "",
        ".",
        "..",
        "models/foo",
        "x:countTokens",
        "x?key=bad",
        "x#f",
        "%2f",
        "x\\y",
        "*",
        "中文",
    ] {
        assert!(!valid_model(model), "accepted {model}");
    }
    assert!(!valid_model(&"x".repeat(2049)));
}
#[test]
fn vertex_express_ownership_does_not_capture_legacy_project_vertex_or_aistudio() {
    assert!(owns_base(BASE_URL));
    assert!(owns_base(
        "https://aiplatform.googleapis.com/v1beta1/publishers/other"
    ));
    for base in ["https://aiplatform.googleapis.com/v1", "https://us-central1-aiplatform.googleapis.com/v1/projects/p/locations/us-central1/publishers/google", "https://generativelanguage.googleapis.com/v1beta", "https://aiplatform.googleapis.com.evil.invalid/v1/publishers/google"] {
        assert!(!owns_base(base), "captured {base}");
    }
}
#[cfg(feature = "line-google-agent-platform-official")]
#[test]
fn vertex_express_configuration_accepts_empty_pool_clear_and_static_key_without_refresh() {
    let mut empty = provider();
    empty["credentials"] = json!([]);
    assert!(compile(empty).unwrap().snapshot().get_providers()[0]
        .credential_pool
        .is_empty());
    let mut clear = provider();
    clear["credentials"][0]["api_key"] = json!("");
    compile(clear).unwrap();
    let store = compile(provider()).unwrap();
    let snapshot = store.snapshot();
    let compiled = &snapshot.get_providers()[0];
    assert_eq!(compiled.protocol_profile, PROFILE);
    assert_eq!(compiled.protocol_family, "gemini_generate_content");
    assert!(compiled.credential_pool[0].refresh_config.is_none());
    assert_eq!(
        crate::implementation_lines::line_for_payload(&payload()),
        Some(
            crate::implementation_lines::RefactoredImplementationLine::GoogleAgentPlatformOfficial
        )
    );
}
#[cfg(feature = "line-google-agent-platform-official")]
#[test]
fn vertex_express_original_preset_and_final_merge_cannot_escape_product() {
    for (name, value) in [
        ("auth_mode", json!("bearer")),
        ("adapter", json!("openai_compatible")),
        (
            "base_url",
            json!("https://generativelanguage.googleapis.com/v1beta"),
        ),
        (
            "chat_completions_path",
            json!("/projects/p/locations/global/models/{model}"),
        ),
        ("protocol_profile", json!("antigravity_oauth")),
        ("call_flow", json!({"call_adapter":"openai"})),
        ("request_client_profile", json!("codex_cli")),
        ("headers", json!({"X-Goog-User-Project":"other"})),
    ] {
        let mut bad = provider();
        bad[name] = value;
        assert!(compile(bad).is_err(), "accepted provider {name}");
    }
    for (name, value) in [
        ("auth_mode", json!("bearer")),
        ("base_url", json!("https://proxy.invalid")),
        ("auth_token", json!("Bearer oauth")),
        ("refresh_token", json!("rt")),
        (
            "refresh_endpoint",
            json!("https://oauth2.googleapis.com/token"),
        ),
        ("refresh_client_id", json!("client")),
        ("token_expires_in_secs", json!(3600)),
        ("session_auth", json!({"transport":"bearer"})),
        ("headers", json!({"Cookie":"session"})),
        ("extra_body", json!({"project":"other"})),
        ("execution_mode", json!("browser_backed")),
    ] {
        let mut bad = provider();
        bad["credentials"][0][name] = value;
        assert!(compile(bad).is_err(), "accepted credential {name}");
    }
    let mut erased = provider();
    erased["auth_mode"] = json!("bearer");
    erased["base_url"] = json!("https://proxy.invalid");
    assert!(compile(erased).is_err());
    // ProviderConfigYaml has no auth_header_name override; unknown YAML fields cannot change the preset.
    let mut ignored = provider();
    ignored["auth_header_name"] = json!("authorization");
    let store = compile(ignored).unwrap();
    assert_eq!(
        store.snapshot().get_providers()[0].credential_pool[0]
            .payload
            .auth_header_name
            .as_deref(),
        Some("x-goog-api-key")
    );
}
#[cfg(feature = "line-google-agent-platform-official")]
#[test]
fn vertex_express_preparation_and_final_plan_reject_late_mutations() {
    let request = crate::protocol::openai::normalize_chat_completions(
        json!({"messages":[{"role":"user","content":"hello"}]}),
    )
    .unwrap();
    let p = payload();
    let mut raw_override = p.clone();
    raw_override.auth_header_name = Some("authorization".into());
    assert!(crate::protocol::call_flow::prepare(&raw_override).is_err());
    for stream in [false, true] {
        let plan = crate::upstream::client::UpstreamClient::build_request_plan(
            &p,
            &request,
            "gemini-contract",
            stream,
        )
        .unwrap();
        validate_plan(&p, &plan).unwrap();
        assert_eq!(
            plan.url,
            format!(
                "{BASE_URL}/models/gemini-contract:{}",
                if stream {
                    "streamGenerateContent"
                } else {
                    "generateContent"
                }
            )
        );
        for kind in 0..4 {
            let mut bad = crate::upstream::client::UpstreamClient::build_request_plan(
                &p,
                &request,
                "gemini-contract",
                stream,
            )
            .unwrap();
            match kind {
                0 => {
                    bad.url = bad
                        .url
                        .replace("aiplatform.googleapis.com", "proxy.invalid")
                }
                1 => bad.query.push(("key".into(), "other".into())),
                2 => bad.method = rquest::Method::GET,
                _ => bad.url = bad.url.replace("gemini-contract", "..%2fmodels%2fother"),
            }
            assert!(validate_plan(&p, &bad).is_err());
        }
    }
    for model in [
        "publishers/google/models/gemini-contract",
        "x?key=bad",
        "x#secret",
        "x/../y",
    ] {
        assert!(crate::upstream::client::UpstreamClient::build_request_plan(
            &p, &request, model, true
        )
        .is_err());
    }
    let mut changed = p.clone();
    changed.call_flow = Some(serde_json::from_value(json!({"call_adapter":"openai"})).unwrap());
    assert!(crate::protocol::call_flow::prepare(&changed).is_err());
}
#[cfg(feature = "line-google-agent-platform-official")]
#[test]
fn vertex_express_final_header_owner_removes_caller_bearer_cookie_and_host() {
    let request = crate::protocol::openai::normalize_chat_completions(
        json!({"messages":[{"role":"user","content":"hello"}]}),
    )
    .unwrap();
    let p = payload();
    let plan = crate::upstream::client::UpstreamClient::build_request_plan(
        &p,
        &request,
        "gemini-contract",
        true,
    )
    .unwrap();
    let mut headers = rquest::header::HeaderMap::new();
    for name in [
        "authorization",
        "cookie",
        "host",
        "x-goog-user-project",
        "x-goog-api-key",
    ] {
        headers.insert(name, rquest::header::HeaderValue::from_static("untrusted"));
    }
    crate::upstream::vertex_express::finalize_headers(&p, &plan, &mut headers).unwrap();
    assert_eq!(headers.len(), 3);
    assert_eq!(headers["x-goog-api-key"], "synthetic-express-key");
    let mut empty = p;
    empty.api_key.clear();
    assert!(validate_plan(&empty, &plan).is_err());
}
#[cfg(not(feature = "line-google-agent-platform-official"))]
#[test]
fn vertex_express_feature_off_refuses_explicit_marker_and_has_no_preset() {
    let payload:ProviderAccountPayload=serde_json::from_value(json!({"adapter":ADAPTER,"base_url":BASE_URL,"api_key":"synthetic-only","auth_mode":AUTH_MODE,"auth_header_name":"x-goog-api-key"})).unwrap();
    assert_eq!(
        validate(&payload, ADAPTER).unwrap_err().code.as_deref(),
        Some("gateway_provider_line_compiled_out")
    );
    assert!(crate::preset::get_builtin_preset(PRESET_ID).is_none());
    assert!(compile(provider()).is_err());
}
