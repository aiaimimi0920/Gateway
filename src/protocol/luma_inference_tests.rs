//! Luma token inference is a distinct account audience, not the Agents media API.
use crate::{preset, protocol::openai, upstream::client::UpstreamClient};
use serde_json::json;

fn payload() -> crate::routing::candidate::ProviderAccountPayload {
    preset::compile_provider_account(
        &preset::get_builtin_preset("lumalabs-inference-api").unwrap(),
        &serde_json::from_value(
            json!({"base_url":"https://inference.lumalabs.ai/v1", "api_key":"inference_synthetic"}),
        )
        .unwrap(),
    )
}

#[test]
fn luma_inference_has_its_own_key_preset_and_chat_plan() {
    let preset = preset::get_builtin_preset("lumalabs-inference-api")
        .expect("independent Luma LLM inference preset");
    let payload = preset::compile_provider_account(
        &preset,
        &serde_json::from_value(json!({"base_url":"https://inference.lumalabs.ai/v1",
            "api_key":"inference_synthetic"}))
        .unwrap(),
    );
    let req = openai::normalize_chat_completions(json!({"model":"glm-5.2",
        "messages":[{"role":"user","content":"hello"}]}))
    .unwrap();
    let plan = UpstreamClient::build_request_plan(&payload, &req, "glm-5.2", false).unwrap();
    assert_eq!(
        plan.url,
        "https://inference.lumalabs.ai/v1/chat/completions"
    );
    assert_ne!(payload.adapter, crate::protocol::luma_agents::ADAPTER);
    assert!(payload.session_auth.is_none() && preset.default_model.is_none());
}

#[test]
fn luma_inference_rejects_cross_product_keys_targets_and_unsupported_requests() {
    use crate::protocol::{call_flow, luma_inference as luma};
    let original = payload();
    for (field, value) in [
        ("base_url", json!("https://agents.lumalabs.ai/v1")),
        ("base_url", json!("https://foreign.invalid/v1")),
        (
            "base_url",
            json!("https://inference.lumalabs.ai/v1?secret=x"),
        ),
        ("api_key", json!("luma-api-media-key")),
        ("auth_mode", json!("bearer")),
        ("headers", json!({"cookie":"synthetic"})),
        ("responses_path", json!("/responses")),
        (
            "call_flow",
            json!({"call_adapter":"auto","fingerprint_adapter":"codex_cli","special_adapter":"auto"}),
        ),
    ] {
        let mut bad = serde_json::to_value(&original).unwrap();
        bad[field] = value;
        let bad = serde_json::from_value(bad).unwrap();
        assert!(call_flow::prepare(&bad).is_err(), "{field}");
    }
    let mut req = openai::normalize_chat_completions(
        json!({"model":"glm-5.2","messages":[{"role":"user","content":"hello"}]}),
    )
    .unwrap();
    for endpoint in [
        crate::protocol::canonical::EndpointKind::Embeddings,
        crate::protocol::canonical::EndpointKind::ImagesGenerations,
    ] {
        req.endpoint_kind = endpoint;
        assert!(luma::validate_request(&original, &req).is_err());
    }
    req.endpoint_kind = crate::protocol::canonical::EndpointKind::Responses;
    req.previous_response_id = Some("foreign-response".into());
    assert!(luma::validate_request(&original, &req).is_err());
}

#[test]
fn luma_inference_normal_configuration_and_keep_are_bound_to_the_exact_product() {
    use crate::console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch};
    use crate::routing::config::{RouteConfigStore, RouteConfigYaml};
    let document = json!({"providers":[{"id":"luma-llm","preset":"lumalabs-inference-api","base_url":"https://inference.lumalabs.ai/v1",
        "explicit_credentials_only":true,"credentials":[{"id":"one","api_key":"inference_synthetic","supported_models":["glm-5.2"]}]}]});
    let active: RouteConfigYaml = serde_json::from_value(document.clone()).unwrap();
    assert!(RouteConfigStore::from_document(active.clone()).is_ok());
    for patch in [
        json!({"base_url":"https://agents.lumalabs.ai/v1"}),
        json!({"preset":"openai"}),
        json!({"credentials":[{"id":"one","api_key":"luma-api-synthetic"}]}),
        json!({"credentials":[{"id":"one","api_key":"inference_synthetic","refresh_token":"synthetic"}]}),
        json!({"credentials":[{"id":"one","auth_mode":"bearer","api_key":"inference_synthetic"}]}),
    ] {
        let mut bad = document.clone();
        bad["providers"][0]
            .as_object_mut()
            .unwrap()
            .extend(patch.as_object().unwrap().clone());
        assert!(
            RouteConfigStore::from_document(serde_json::from_value(bad).unwrap()).is_err(),
            "{patch}"
        );
    }
    let redacted = redact_route_document(&active).unwrap();
    let patches = [SecretPatch::keep("/providers/0/credentials/0/api_key")];
    let mut metadata = redacted.document.clone();
    metadata.providers[0].label = Some("renamed".into());
    let saved = resolve_secret_patches(&active, metadata, &patches).unwrap();
    assert_eq!(
        saved.providers[0].credentials[0].api_key.as_deref(),
        Some("inference_synthetic")
    );
    let mut foreign = redacted.document;
    foreign.providers[0].base_url = "https://agents.lumalabs.ai/v1".into();
    assert_eq!(
        resolve_secret_patches(&active, foreign, &patches)
            .unwrap_err()
            .code(),
        "secret_keep_destination_changed"
    );
}
