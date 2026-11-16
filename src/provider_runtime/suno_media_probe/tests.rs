//! Real loopback generation and MP3 bytes through the normal one-account model-test owner.
#[path = "failure_tests.rs"]
mod failure_tests;
#[path = "../../upstream/client/tests/suno_pipeline/fixture.rs"]
mod fixture;
#[path = "management_tests.rs"]
mod management_tests;
#[path = "../../upstream/suno/tests/fixture.rs"]
mod peer;
#[path = "../../../tests/console_contract_support/mod.rs"]
mod support;
use crate::provider_runtime::{
    probe_console_target_with_request, ConsoleProbeRequest, ProviderPayloadProbeStatus,
};
use fixture::{Fixture, MODEL, NATIVE, SESSION};
use peer::{clip, Reply, Server};
use serde_json::{json, Value};

fn options() -> Value {
    json!({"prompt":"合成音乐测试","model":MODEL,"credentialIds":["exact"],
        "media":{"operation":"music","confirmCharge":true,"parameters":{}}})
}
fn parsed(input: Value) -> ConsoleProbeRequest {
    ConsoleProbeRequest::parse(input.to_string().as_bytes())
        .unwrap()
        .unwrap()
}
fn completed() -> Vec<Reply> {
    vec![
        Reply::json(json!({"required":false})),
        Reply::json(json!({"clips":[clip("one","complete"),clip("two","complete")]})),
        Reply::audio(),
        Reply::audio(),
    ]
}
async fn run(
    fixture: &Fixture,
    input: Option<&ConsoleProbeRequest>,
) -> crate::provider_runtime::ProviderPayloadProbeReport {
    let target = fixture
        .state
        .route_config
        .snapshot()
        .select_credential_probe_target("exact")
        .unwrap();
    Box::pin(probe_console_target_with_request(
        &fixture.state,
        &target,
        input,
    ))
    .await
    .1
}
fn closed_audit(row: &crate::db::GatewayRequestAuditView, verified: bool) {
    assert_eq!(row.protocol_family, "suno_web");
    assert_eq!(row.endpoint_kind, "music_generations");
    assert_eq!(row.requested_model.as_deref(), Some(MODEL));
    assert_eq!(row.resolved_model.as_deref(), Some(NATIVE));
    assert_eq!(row.model_alias.as_deref(), Some(MODEL));
    assert!(row.total_tokens.is_none() && row.access_key_id.is_none());
    let trace = row.route_trace.as_ref().unwrap();
    assert_eq!(trace["realCredentialRef"], "exact");
    assert_eq!(
        trace["consoleMediaProbe"],
        json!({"operation":"music","chargeConfirmed":true})
    );
    if verified {
        assert_eq!(
            trace["sunoMusic"],
            json!({"verified":true,"count":2,"wireModel":crate::protocol::suno::SUNO_DEFAULT_UPSTREAM_MODEL,"supplierCharge":null})
        );
    } else {
        assert!(trace.get("sunoMusic").is_none());
    }
    let raw = serde_json::to_string(row).unwrap();
    for private in [
        SESSION,
        "track.mp3",
        "合成音乐测试",
        "audio_url",
        "lyric",
        "private signed-token",
    ] {
        assert!(!raw.contains(private), "{raw}");
    }
}
#[tokio::test]
async fn suno_probe_exact_account_decodes_all_music_and_records_closed_unknown_charge_audit() {
    Box::pin(async {
        let peer = Server::start(completed()).await;
        let fixture = Box::pin(Fixture::new(&peer)).await;
        let before =
            serde_json::to_value(fixture.state.route_config.snapshot().document()).unwrap();
        let report = crate::upstream::public_assets::within(
            &peer.origin,
            run(&fixture, Some(&parsed(options()))),
        )
        .await;
        assert_eq!(
            report.status,
            ProviderPayloadProbeStatus::Passed,
            "{}",
            report.message
        );
        assert!(report.answer.is_none() && report.health_recovery.is_some());
        assert!(report.message.contains("已验证 2 首 MP3") && report.message.contains("扣额未知"));
        assert!(report
            .message
            .contains(crate::protocol::suno::SUNO_DEFAULT_UPSTREAM_MODEL));
        let row = fixture.audit().await;
        assert_eq!(row.status, "completed");
        closed_audit(&row, true);
        assert_eq!(fixture.balance().await.remaining_messages, Some(2));
        let states = fixture
            .state
            .local_runtime
            .as_ref()
            .unwrap()
            .list_model_states(Default::default())
            .await
            .unwrap();
        assert_eq!(states.len(), 1, "{states:?}");
        assert_eq!(states[0].model, NATIVE);
        assert_eq!(states[0].provider_credential_ref.as_deref(), Some("exact"));
        assert_eq!(states[0].failure_count, 0);
        assert_eq!(
            serde_json::to_value(fixture.state.route_config.snapshot().document()).unwrap(),
            before
        );
        fixture::wire(&peer, 2);
        fixture.finish().await;
        peer.stop().await;
    })
    .await;
}

#[tokio::test]
async fn suno_probe_default_and_explicit_text_are_unsupported_without_generation_or_assets() {
    Box::pin(async {
        let peer = Server::start(vec![]).await;
        let fixture = Box::pin(Fixture::new(&peer)).await;
        let text = parsed(json!({"prompt":"OK","model":MODEL,"credentialIds":["exact"]}));
        for input in [None, Some(&text)] {
            let report = run(&fixture, input).await;
            assert_eq!(report.status, ProviderPayloadProbeStatus::Unsupported);
        }
        assert!(peer.calls().is_empty());
        assert_eq!(fixture.balance().await.remaining_messages, Some(2));
        assert!(fixture
            .state
            .local_runtime
            .as_ref()
            .unwrap()
            .list_audits(&Default::default())
            .await
            .unwrap()
            .is_empty());
        fixture.finish().await;
        peer.stop().await;
    })
    .await;
}

#[test]
fn suno_probe_music_request_is_additive_but_requires_ephemeral_cost_and_exact_account() {
    assert!(ConsoleProbeRequest::parse(options().to_string().as_bytes()).is_ok());
    for change in [
        json!({"credentialIds":["exact","other"]}),
        json!({"media":{"operation":"music","confirmCharge":false}}),
        json!({"media":{"operation":"music","confirmCharge":true,"parameters":{"model":"other"}}}),
        json!({"scope":{"kind":"pool"}}),
        json!({"planId":"saved"}),
    ] {
        let mut input = options();
        input
            .as_object_mut()
            .unwrap()
            .extend(change.as_object().unwrap().clone());
        assert!(
            ConsoleProbeRequest::parse(input.to_string().as_bytes()).is_err(),
            "{input}"
        );
    }
}
