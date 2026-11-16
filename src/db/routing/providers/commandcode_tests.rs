//! Hosted row/credential readers and final routing, without a live PostgreSQL service.
use super::*;
use crate::protocol::{commandcode, registry};
use serde_json::json;

#[test]
fn commandcode_hosted_shared_reader_and_cache_prepare_restore_the_same_catalog() {
    let source = raw("/messages");
    let parsed = crate::routing::candidate::deserialize_provider_payload(&source, None).unwrap();
    assert_eq!(
        commandcode::catalog::families(&parsed, Some("upstream")).unwrap(),
        [registry::ANTHROPIC_MESSAGES_FAMILY]
    );
    let cached = serde_json::from_value(source.clone()).unwrap();
    let prepared = prepare_route_payload(
        cached,
        Some(&source),
        registry::OPENAI_CHAT_FAMILY,
        Some("alias"),
        Some("upstream"),
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        prepared.supported_protocol_families,
        [registry::ANTHROPIC_MESSAGES_FAMILY]
    );
    assert_eq!(parsed.extra_body, prepared.payload.extra_body);
    // Rehydrating an already compiled payload is idempotent, not a second merge.
    let again = prepare_route_payload(
        prepared.payload,
        Some(&source),
        registry::OPENAI_CHAT_FAMILY,
        Some("alias"),
        Some("upstream"),
    )
    .unwrap()
    .unwrap();
    assert_eq!(parsed.extra_body, again.payload.extra_body);
}

#[tokio::test]
async fn commandcode_hosted_credential_catalog_is_an_atomic_snapshot() {
    let mut account = raw("/chat/completions");
    account["model_catalog"]["models"] = json!(["old-account-model"]);
    account["model_catalog"]["supportedEndpoints"] =
        json!({"old-account-model":["/chat/completions"]});
    let credential = super::super::GatewayProviderCredentialView {
        id: "exact".into(),
        provider_account_id: "commandcode-hosted".into(),
        label: "fixture".into(),
        status: "active".into(),
        payload: json!({"api_key":"synthetic-credential-key","model_catalog":raw("/messages")["model_catalog"]}),
        storage_mode: "inline".into(),
        source_kind: "manual".into(),
        source_path: None,
        source_hash: None,
        sync_mode: "manual".into(),
        sync_state: "ready".into(),
        sync_error: None,
        cooldown_until: None,
        last_error: None,
        failure_count: 0,
        last_health_check_at: None,
        created_at: String::new(),
        updated_at: String::new(),
        archived_at: None,
    };
    let credentials = vec![credential];
    let selected = build_route_candidates_for_provider(
        &row(account),
        Some(&credentials),
        EndpointKind::Messages,
        Some("alias".into()),
        Some("upstream".into()),
        1,
        1,
    )
    .await
    .unwrap();
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].provider_credential_id.as_deref(), Some("exact"));
    assert_eq!(selected[0].payload.api_key, "synthetic-credential-key");
    assert_eq!(
        selected[0].supported_protocol_families,
        [registry::ANTHROPIC_MESSAGES_FAMILY]
    );
    assert!(!serde_json::to_string(&selected[0].payload.extra_body)
        .unwrap()
        .contains("old-account-model"));
}

fn raw(path: &str) -> Value {
    json!({"adapter":"openai_compatible","auth_mode":commandcode::AUTH_MODE,
        "base_url":commandcode::BASE,"api_key":"synthetic-provider-key",
        "model_catalog":{"sourceUrl":commandcode::SOURCE,"checkedAt":"2026-11-14T00:00:00Z",
            "models":["upstream"],"supportedEndpoints":{"upstream":[path]}}})
}

#[test]
fn commandcode_hosted_actual_cached_reader_restores_identity_and_rejects_blocks() {
    for preset in [false, true] {
        for key in ["auth_mode", "authMode"] {
            let mut source = raw("/messages");
            source.as_object_mut().unwrap().remove("auth_mode");
            source[key] = json!(commandcode::AUTH_MODE);
            if preset {
                source["preset"] = json!(commandcode::PRESET);
            }
            let mut credential: crate::redis::credential_cache::CredentialEntry = serde_json::from_value(json!({
                "id":"cached-exact","kind":"account_credential","project_id":"fixture","user_id":"fixture",
                "provider":"openai","api_key":"synthetic-cached-key","api_base_url":commandcode::BASE,
                "account_payload":source,"created_at":"","updated_at":""
            })).unwrap();
            let candidate = crate::routing::credential_routing::credential_to_candidate(
                &credential,
                "upstream",
            )
            .unwrap();
            assert!(commandcode::owns_payload(&candidate.payload));
            assert_eq!(candidate.payload.api_key, "synthetic-cached-key");
            assert_eq!(
                candidate.supported_protocol_families,
                [registry::ANTHROPIC_MESSAGES_FAMILY]
            );
            credential.account_payload.as_mut().unwrap()["blocked_protocol_families"] =
                json!([registry::ANTHROPIC_MESSAGES_FAMILY]);
            assert!(crate::routing::credential_routing::credential_to_candidate(
                &credential,
                "upstream"
            )
            .is_none());
        }
    }
}

#[test]
fn commandcode_hosted_identity_conflicts_and_duplicate_aliases_are_not_sanitized() {
    let mut source = raw("/messages");
    source["authMode"] = json!("other");
    assert!(crate::routing::candidate::deserialize_provider_payload(&source, None).is_err());
    source.as_object_mut().unwrap().remove("authMode");
    source["modelCatalog"] = source["model_catalog"].clone();
    assert!(crate::routing::candidate::deserialize_provider_payload(&source, None).is_err());
    source.as_object_mut().unwrap().remove("modelCatalog");
    let mut parsed: ProviderAccountPayload = serde_json::from_value(source.clone()).unwrap();
    parsed.auth_mode = Some("other".into());
    assert!(commandcode::catalog::hydrate(&mut parsed, Some(&source)).is_err());
}
fn row(raw: Value) -> GatewayProviderRouteRow {
    GatewayProviderRouteRow {
        id: "commandcode-hosted".into(),
        label: "fixture".into(),
        adapter: "openai_compatible".into(),
        protocol_family: registry::OPENAI_CHAT_FAMILY.into(),
        protocol_profile: commandcode::PROFILE.into(),
        cooldown_until: None,
        failure_count: 0,
        execution_mode: "direct_http".into(),
        endpoint_execution_modes: None,
        payload_inline: Some(Json(raw)),
        payload_object_key: None,
    }
}
async fn candidates(raw: Value) -> Vec<RouteCandidate> {
    build_route_candidates_for_provider(
        &row(raw),
        None,
        EndpointKind::Responses,
        Some("alias".into()),
        Some("upstream".into()),
        1,
        1,
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn commandcode_hosted_catalog_selects_native_wire_without_forwarding_metadata() {
    for (path, family) in [
        ("/messages", registry::ANTHROPIC_MESSAGES_FAMILY),
        ("/responses", registry::OPENAI_RESPONSES_FAMILY),
    ] {
        let mut candidates = candidates(raw(path)).await;
        assert_eq!(candidates.len(), 1);
        let candidate = &mut candidates[0];
        assert_eq!(candidate.supported_protocol_families, [family]);
        let request = if path == "/messages" {
            crate::protocol::anthropic::normalize_messages(json!({"model":"alias","max_tokens":32,
                "messages":[{"role":"user","content":"hello"}]}))
            .unwrap()
        } else {
            crate::protocol::responses::normalize_responses(
                json!({"model":"alias","input":"hello"}),
            )
            .unwrap()
        };
        assert_eq!(
            crate::routing::protocol_resolution::finalize_candidate_protocol_family(
                candidate, &request
            ),
            Some(true)
        );
        let plan = crate::upstream::client::UpstreamClient::build_request_plan(
            &candidate.payload,
            &request,
            "upstream",
            false,
        )
        .unwrap();
        assert_eq!(plan.url, format!("{}{path}", commandcode::BASE));
        assert_eq!(candidate.payload.api_key, "synthetic-provider-key");
        let body = plan.body.unwrap().to_string();
        assert!(!body.contains("model_catalog") && !body.contains("gateway_commandcode_catalog"));
    }
}

#[tokio::test]
async fn commandcode_hosted_catalog_obeys_global_and_model_family_policies() {
    for (path, family) in [
        ("/messages", registry::ANTHROPIC_MESSAGES_FAMILY),
        ("/responses", registry::OPENAI_RESPONSES_FAMILY),
    ] {
        for (key, value, allowed) in [
            ("allowed_protocol_families", json!([family]), true),
            (
                "allowed_protocol_families",
                json!([registry::OPENAI_CHAT_FAMILY]),
                false,
            ),
            ("blocked_protocol_families", json!([family]), false),
            (
                "allowed_protocol_families_by_model",
                json!({"alias":[family]}),
                true,
            ),
            (
                "blocked_protocol_families_by_model",
                json!({"upstream":[family]}),
                false,
            ),
            (
                "blockedProtocolFamiliesByModel",
                json!({"ali*":[family]}),
                false,
            ),
        ] {
            let mut payload = raw(path);
            payload[key] = value;
            assert_eq!(
                !candidates(payload).await.is_empty(),
                allowed,
                "{path}: {key}"
            );
        }
    }
}

#[test]
fn commandcode_hosted_catalog_rejects_invalid_raw_metadata_and_body_overrides() {
    for (key, value) in [
        (
            "model_catalog",
            json!({"sourceUrl":"https://foreign.test/models","models":["upstream"]}),
        ),
        ("model_catalog", json!("malformed")),
        ("extra_body", json!({"untrusted":true})),
    ] {
        let mut payload = raw("/messages");
        payload[key] = value;
        let provider = row(payload.clone());
        assert!(
            provider_payload_from_value(&provider, payload).is_err(),
            "{key}"
        );
    }
}

#[tokio::test]
async fn commandcode_hosted_legacy_and_manual_models_keep_chat_only() {
    let mut legacy = raw("/messages");
    legacy.as_object_mut().unwrap().remove("model_catalog");
    for payload in [legacy, raw("/messages")] {
        let candidates = build_route_candidates_for_provider(
            &row(payload),
            None,
            EndpointKind::ChatCompletions,
            Some("manual".into()),
            Some("manual".into()),
            1,
            1,
        )
        .await
        .unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].supported_protocol_families,
            [registry::OPENAI_CHAT_FAMILY]
        );
    }
}
