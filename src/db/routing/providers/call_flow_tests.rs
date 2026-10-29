//! Pure Hosted row fixtures exercise the production reader/builder, without PostgreSQL.
use super::*;
use serde_json::json;

fn row(payload: Value) -> GatewayProviderRouteRow {
    GatewayProviderRouteRow {
        id: "fixture".into(),
        label: "fixture".into(),
        adapter: "openai_compatible".into(),
        protocol_family: "openai_chat".into(),
        protocol_profile: "openai_compatible_generic".into(),
        cooldown_until: None,
        failure_count: 0,
        execution_mode: "direct_http".into(),
        endpoint_execution_modes: None,
        payload_inline: Some(Json(payload)),
        payload_object_key: None,
    }
}

#[tokio::test]
async fn call_flow_hosted_builder_uses_effective_adapter_before_model_policy() {
    let raw = json!({"adapter":"openai_compatible","base_url":"https://fixture.test","api_key":"fixture",
        "call_flow":{"call_adapter":"anthropic","fingerprint_adapter":"claude_cli","special_adapter":"none"},
        "allowed_protocol_families_by_model":{"fixture":["anthropic_messages"]}});
    let provider = row(raw.clone());
    let candidates = build_route_candidates_for_provider(
        &provider,
        None,
        EndpointKind::Responses,
        Some("fixture".into()),
        Some("upstream".into()),
        1,
        1,
    )
    .await
    .unwrap();
    assert_eq!(candidates.len(), 1);
    let candidate = &candidates[0];
    assert_eq!(candidate.adapter, "anthropic_compatible");
    assert_eq!(candidate.payload.adapter, candidate.adapter);
    assert_eq!(candidate.protocol_family, "anthropic_messages");
    assert_eq!(
        candidate.supported_protocol_families,
        ["anthropic_messages"]
    );
    assert_eq!(
        candidate.resolved_execution_mode,
        ProviderExecutionMode::DirectHttp
    );

    for rule in [
        "blocked_protocol_families_by_model",
        "allowed_protocol_families_by_model",
    ] {
        let mut blocked = raw.clone();
        blocked[rule] = json!({"fixture":[if rule.starts_with("blocked") { "anthropic_messages" } else { "openai_responses" }]});
        let candidates = build_route_candidates_for_provider(
            &row(blocked),
            None,
            EndpointKind::Responses,
            Some("fixture".into()),
            Some("upstream".into()),
            1,
            1,
        )
        .await
        .unwrap();
        assert!(candidates.is_empty(), "rule={rule}");
    }
}

#[test]
fn call_flow_hosted_payload_reader_rejects_invalid_special_executor() {
    let raw = json!({"adapter":"freebuff_compatible","base_url":"https://fixture.test","api_key":"fixture",
        "call_flow":{"call_adapter":"anthropic","special_adapter":"none"}});
    let mut provider = row(raw.clone());
    provider.adapter = "freebuff_compatible".into();
    let error = provider_payload_from_value(&provider, raw).unwrap_err();
    assert!(error.message.starts_with("Invalid call_flow:"));
}

#[tokio::test]
async fn call_flow_hosted_credential_merge_retains_policy_and_chosen_secret() {
    let provider = row(
        json!({"adapter":"openai_compatible","base_url":"https://fixture.test","api_key":"provider-fixture"}),
    );
    let mut credential = super::super::GatewayProviderCredentialView {
        id: "credential-fixture".into(),
        provider_account_id: "fixture".into(),
        label: "fixture".into(),
        status: "active".into(),
        payload: json!({"api_key":"chosen-fixture",
            "call_flow":{"call_adapter":"anthropic","fingerprint_adapter":"claude_cli","special_adapter":"none"}}),
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
    let build = |credential| {
        build_route_candidates_for_provider(
            &provider,
            credential,
            EndpointKind::Responses,
            Some("fixture".into()),
            Some("upstream".into()),
            1,
            1,
        )
    };
    let credentials = vec![credential.clone()];
    let candidates = build(Some(&credentials)).await.unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].adapter, "anthropic_compatible");
    assert_eq!(candidates[0].payload.api_key, "chosen-fixture");
    assert_eq!(
        candidates[0].provider_credential_id.as_deref(),
        Some("credential-fixture")
    );
    // The legacy credential override must win over an inherited provider flow,
    // just as it does in YAML compilation, before both routing readers prepare it.
    let mut inherited = provider.clone();
    inherited.payload_inline.as_mut().unwrap().0["call_flow"] =
        credential.payload["call_flow"].clone();
    let mut override_credential = credential.clone();
    override_credential.payload =
        json!({"api_key":"override-fixture","request_client_profile":"codex_cli"});
    let overrides = vec![override_credential];
    let overridden = build_route_candidates_for_provider(
        &inherited,
        Some(&overrides),
        EndpointKind::Responses,
        Some("fixture".into()),
        Some("upstream".into()),
        1,
        1,
    )
    .await
    .unwrap();
    assert_eq!(
        overridden[0].payload.request_client_profile,
        Some(crate::protocol::request_profile::RequestClientProfile::CodexCli)
    );
    assert_eq!(overridden[0].payload.api_key, "override-fixture");
    credential.payload["blocked_protocol_families_by_model"] =
        json!({"fixture":["anthropic_messages"]});
    let credentials = vec![credential];
    assert!(build(Some(&credentials)).await.unwrap().is_empty());
}
