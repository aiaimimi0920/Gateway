//! One exact Luma Web account can prove image bytes, not the supplier's model selection.
#[path = "lumalabs_probe/boundaries.rs"]
mod boundaries;
#[path = "lumalabs_probe/browser_host.rs"]
mod browser_host;
#[path = "lumalabs_pipeline/fixture.rs"]
mod fixture;
#[path = "lumalabs_probe/management.rs"]
mod management;
#[path = "lumalabs_pipeline/peer.rs"]
mod peer;
use crate::provider_runtime::{
    probe_console_target_with_request, ConsoleProbeRequest, ProviderPayloadProbeStatus,
};
use crate::upstream::public_assets::within;
use fixture::{Fixture, MODEL, NATIVE};
use peer::{Peer, Reply, ASSET};
use serde_json::{json, Value};

fn options() -> Value {
    json!({"prompt":"synthetic private image prompt","model":MODEL,"credentialIds":["exact"],
        "media":{"operation":"image","confirmCharge":true,"parameters":{}}})
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
    let request = input.map(|value| {
        ConsoleProbeRequest::parse(value.to_string().as_bytes())
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
fn closed_audit(row: &crate::db::GatewayRequestAuditView, verified: bool) {
    assert_eq!(row.protocol_family, "lumalabs_web");
    assert_eq!(row.endpoint_kind, "images_generations");
    assert_eq!(row.requested_model.as_deref(), Some(MODEL));
    assert_eq!(row.resolved_model.as_deref(), Some(NATIVE));
    assert_eq!(row.model_alias.as_deref(), Some(MODEL));
    assert!(row.total_tokens.is_none() && row.access_key_id.is_none());
    let trace = row.route_trace.as_ref().unwrap();
    assert_eq!(trace["realCredentialRef"], "exact");
    assert_eq!(
        trace["consoleMediaProbe"],
        json!({"operation":"image","chargeConfirmed":true})
    );
    if verified {
        assert_eq!(
            trace["lumaWebImage"],
            json!({"verified":true,"count":1,"modelVerified":false,"supplierCharge":null})
        );
    } else {
        assert!(trace.get("lumaWebImage").is_none());
    }
    let raw = serde_json::to_string(row).unwrap();
    for private in [
        ASSET,
        "synthetic private image prompt",
        "luma-synthetic-exact",
        "synthetic-realm",
    ] {
        assert!(!raw.contains(private), "audit leaked {private}");
    }
}

#[tokio::test]
async fn luma_web_probe_verified_image_is_neutral_without_model_health_or_key_usage() {
    Box::pin(async {
        let peer = Peer::start(vec![
            Reply::executor(ASSET),
            Reply::image(image::ImageFormat::Png),
        ])
        .await;
        let fixture = Box::pin(Fixture::new(&peer)).await;
        let before =
            serde_json::to_value(fixture.state.route_config.snapshot().document()).unwrap();
        let report = within(&peer.origin, run(&fixture, Some(options()))).await;
        assert_eq!(
            report.status,
            ProviderPayloadProbeStatus::Unsupported,
            "{}",
            report.message
        );
        assert!(
            report.message.contains("已验证 1 张图片"),
            "{}",
            report.message
        );
        assert!(report.message.contains("模型绑定未验证") && report.message.contains("扣额未知"));
        assert!(report.answer.is_none() && report.health_recovery.is_none());
        let row = fixture.audit().await;
        assert_eq!(row.status, "completed");
        closed_audit(&row, true);
        peer::wire(&peer, true);
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
