//! Exact task metadata is observable without downloading audio or claiming playback health.
#[path = "udio_probe/browser_host.rs"]
mod browser_host;
#[path = "udio_pipeline/fixture.rs"]
mod fixture;
#[path = "udio_probe/management.rs"]
mod management;
#[path = "udio_pipeline/peer.rs"]
mod peer;
#[path = "../../../../tests/console_contract_support/mod.rs"]
mod support;
use crate::provider_runtime::{
    probe_console_target_with_request, ConsoleProbeRequest, ProviderPayloadProbeStatus,
};
use fixture::{Fixture, MODEL, NATIVE_MODEL, SESSION};
use peer::Peer;
use serde_json::{json, Value};

fn options() -> Value {
    json!({"prompt":"合成音乐任务测试","model":MODEL,"credentialIds":["exact"],
        "media":{"operation":"music","confirmCharge":true,"parameters":{}}})
}
fn complete() -> Value {
    json!({"ok":true,"result":{"trackIds":["first","second"],"completed":true,
        "songs":[song("first"),song("second")],"message":"private-remote-message"}})
}
fn song(id: &str) -> Value {
    // This URL is deliberately not playable. The probe must not fetch it or mark audio verified.
    json!({"id":id,"status":"complete","finished":true,"song_path":"https://asset.invalid/not-audio",
        "lyrics":"private-lyrics","title":"private-title"})
}
async fn run(
    fixture: &Fixture,
    input: Option<Value>,
) -> crate::provider_runtime::ProviderPayloadProbeReport {
    let target = fixture
        .state
        .route_config
        .snapshot()
        .select_credential_probe_target("exact")
        .unwrap();
    let request = input.map(|input| {
        ConsoleProbeRequest::parse(input.to_string().as_bytes())
            .unwrap()
            .unwrap()
    });
    Box::pin(probe_console_target_with_request(
        &fixture.state,
        &target,
        request.as_ref(),
    ))
    .await
    .1
}
fn closed_audit(row: &crate::db::GatewayRequestAuditView, completed: bool) {
    assert_eq!(row.protocol_family, "udio_web");
    assert_eq!(row.endpoint_kind, "music_generations");
    assert_eq!(row.requested_model.as_deref(), Some(MODEL));
    assert_eq!(row.resolved_model.as_deref(), Some(NATIVE_MODEL));
    assert_eq!(row.model_alias.as_deref(), Some(MODEL));
    assert!(row.total_tokens.is_none() && row.access_key_id.is_none());
    let trace = row.route_trace.as_ref().unwrap();
    assert_eq!(trace["realCredentialRef"], "exact");
    assert_eq!(
        trace["consoleMediaProbe"],
        json!({"operation":"music","chargeConfirmed":true})
    );
    if completed {
        assert_eq!(
            trace["udioMusicTask"],
            json!({"taskCompleted":true,"count":2,"audioVerified":false,"supplierCharge":null})
        );
    } else {
        assert!(trace.get("udioMusicTask").is_none());
    }
    let raw = serde_json::to_string(row).unwrap();
    for private in [
        SESSION,
        "not-audio",
        "合成音乐任务测试",
        "audio_url",
        "private-lyrics",
        "private-title",
        "private-remote-message",
    ] {
        assert!(!raw.contains(private), "audit leaked {private}");
    }
}

#[tokio::test]
async fn udio_probe_completed_tasks_are_neutral_not_audio_health_or_key_usage() {
    Box::pin(async {
        let peer = Peer::start(complete(), 200, false).await;
        let fixture = Box::pin(Fixture::new(&peer)).await;
        let before =
            serde_json::to_value(fixture.state.route_config.snapshot().document()).unwrap();
        let report = run(&fixture, Some(options())).await;
        assert_eq!(
            report.status,
            ProviderPayloadProbeStatus::Unsupported,
            "{}",
            report.message
        );
        assert!(
            report.message.contains("任务终态已确认"),
            "{}",
            report.message
        );
        assert!(report.message.contains("音频未验证") && report.message.contains("扣额未知"));
        assert!(report.answer.is_none() && report.health_recovery.is_none());
        let row = fixture.audit().await;
        assert_eq!(row.status, "completed");
        closed_audit(&row, true);
        fixture::wire(&peer);
        assert_eq!(peer.calls()[0].1["input"]["waitAudio"], true);
        assert_eq!(fixture.balance().await.remaining_messages, Some(2));
        assert!(fixture
            .state
            .local_runtime
            .as_ref()
            .unwrap()
            .list_model_states(Default::default())
            .await
            .unwrap()
            .is_empty());
        assert_eq!(
            serde_json::to_value(fixture.state.route_config.snapshot().document()).unwrap(),
            before
        );
        fixture.finish().await;
        peer.stop().await;
    })
    .await;
}

#[tokio::test]
async fn udio_probe_no_default_text_or_parameter_override_can_generate() {
    Box::pin(async {
        let peer = Peer::start(complete(), 200, false).await;
        let fixture = Box::pin(Fixture::new(&peer)).await;
        for input in [None, Some(json!({"prompt":"OK","model":MODEL,"credentialIds":["exact"]}))] {
            assert_eq!(run(&fixture,input).await.status, ProviderPayloadProbeStatus::Unsupported);
        }
        for change in [json!({"model":"undeclared"}), json!({"model":"x".repeat(129)}),
            json!({"media":{"operation":"video","confirmCharge":true}}),
            json!({"media":{"operation":"music","confirmCharge":true,"parameters":{"wait_audio":false}}}),
            json!({"media":{"operation":"music","confirmCharge":true,"parameters":{"webhook_url":"https://foreign.invalid"}}})] {
            let mut input=options(); input.as_object_mut().unwrap().extend(change.as_object().unwrap().clone());
            assert_eq!(run(&fixture,Some(input)).await.status, ProviderPayloadProbeStatus::Unsupported);
        }
        assert!(peer.calls().is_empty());
        assert!(fixture.state.local_runtime.as_ref().unwrap().list_audits(&Default::default()).await.unwrap().is_empty());
        fixture.finish().await;
        peer.stop().await;
    }).await;
}

#[tokio::test]
async fn udio_probe_partial_pending_and_uncertain_results_fail_without_replay_or_audio_fetch() {
    Box::pin(async {
        for (body,status) in [(json!({"ok":true,"result":{"trackIds":["first","second"],"completed":true,"songs":[song("first")]}}),200),
            (json!({"ok":true,"result":{"trackIds":["first"],"completed":false,"songs":[song("first")]}}),200),
            (json!({"error":"private-remote-message"}),500)] {
            let peer=Peer::start(body,status,false).await;
            let fixture=Box::pin(Fixture::new(&peer)).await;
            let report=run(&fixture,Some(options())).await;
            assert_eq!(report.status,ProviderPayloadProbeStatus::Failed,"{}",report.message);
            assert!(!report.message.contains("private-remote-message"));
            let row=fixture.audit().await; assert_eq!(row.status,"failed"); closed_audit(&row,false);
            fixture::wire(&peer);
            assert_eq!(fixture.balance().await.remaining_messages,Some(2));
            fixture.finish().await; peer.stop().await;
        }
    }).await;
}

#[tokio::test]
async fn udio_probe_drop_cancels_waiting_and_audit_without_health_or_replay() {
    Box::pin(async {
        let peer=Peer::start(complete(),200,true).await;
        let fixture=Box::pin(Fixture::new(&peer)).await;
        let mut future=Box::pin(run(&fixture,Some(options())));
        tokio::select! { biased; _=peer.wait_called()=>{}, report=&mut future=>panic!("unexpected result: {report:?}") }
        drop(future); peer.wait_closed().await;
        let row=fixture.audit().await; assert_eq!(row.status,"cancelled"); closed_audit(&row,false);
        fixture::wire(&peer);
        assert!(fixture.state.local_runtime.as_ref().unwrap().list_model_states(Default::default()).await.unwrap().is_empty());
        assert_eq!(fixture.balance().await.remaining_messages,Some(2));
        fixture.finish().await; peer.stop().await;
    }).await;
}

#[tokio::test]
async fn udio_probe_task_completion_preserves_existing_protected_model_health() {
    Box::pin(async {
        let peer = Peer::start(complete(), 200, false).await;
        let fixture = Box::pin(Fixture::new(&peer)).await;
        let route = fixture.state.route_config.snapshot();
        let target = route.select_credential_probe_target("exact").unwrap();
        let identity = route.model_probe_identity(&target, NATIVE_MODEL).unwrap();
        let local = fixture.state.local_runtime.as_ref().unwrap();
        let old = local
            .model_failure(crate::db::RecordCredentialModelFailureInput {
                provider_account_id: identity.provider_account_id,
                provider_credential_id: identity.provider_credential_id,
                provider_credential_ref: identity.provider_credential_ref,
                protocol_profile: identity.protocol_profile,
                model: NATIVE_MODEL.into(),
                upstream_status: Some(401),
                upstream_retry_after_ms: None,
                error_message: Some("synthetic credential denial".into()),
                classification: crate::provider_failure::classify_provider_failure(
                    Some(401),
                    None,
                    Some("synthetic credential denial"),
                ),
            })
            .await
            .unwrap()
            .unwrap();
        assert!(old.failure_count > 0);
        let before = serde_json::to_value(old).unwrap();
        let report = run(&fixture, Some(options())).await;
        assert_eq!(
            report.status,
            ProviderPayloadProbeStatus::Unsupported,
            "{}",
            report.message
        );
        assert!(report.message.contains("任务终态已确认"));
        assert!(report.health_recovery.is_none());
        let rows = local.list_model_states(Default::default()).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(serde_json::to_value(&rows[0]).unwrap(), before);
        closed_audit(&fixture.audit().await, true);
        fixture.finish().await;
        peer.stop().await;
    })
    .await;
}

#[tokio::test]
async fn udio_probe_wrong_audience_flow_mode_and_native_model_are_rejected_without_network() {
    Box::pin(async {
        let peer = Peer::start(complete(), 200, false).await;
        let fixture = Box::pin(Fixture::new(&peer)).await;
        let original =
            serde_json::to_value(fixture.state.route_config.snapshot().document()).unwrap();
        for case in 0..4 {
            let mut document = original.clone();
            match case {
                0 => document["providers"][0]["base_url"] = json!("https://foreign.invalid"),
                1 => document["providers"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("call_flow")
                    .map(|_| ())
                    .unwrap(),
                2 => document["providers"][0]["execution_mode"] = json!("direct_http"),
                3 => {
                    let model = "x".repeat(129);
                    document["providers"][0]["model_map"][MODEL] = json!(model);
                    document["providers"][0]["credentials"][1]["supported_models"] = json!([model]);
                }
                _ => unreachable!(),
            }
            fixture
                .state
                .route_config
                .replace_document(serde_json::from_value(document).unwrap())
                .unwrap();
            assert_eq!(
                run(&fixture, Some(options())).await.status,
                ProviderPayloadProbeStatus::Unsupported,
                "case {case}"
            );
        }
        assert!(peer.calls().is_empty());
        fixture.finish().await;
        peer.stop().await;
    })
    .await;
}
