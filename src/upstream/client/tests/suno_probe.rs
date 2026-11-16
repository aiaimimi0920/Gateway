//! Exact browser music tests use the bound executor and the same complete MP3 gate.
#[path = "suno_probe/browser_failures.rs"]
mod browser_failures;
#[path = "suno_probe/browser_host.rs"]
mod browser_host;
#[path = "suno_pipeline/fixture.rs"]
mod fixture;
#[path = "../../suno/tests/fixture.rs"]
mod peer;
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
fn completed() -> Vec<Reply> {
    vec![
        Reply::json(
            json!({"ok":true,"result":{"clips":[clip("one","complete"),clip("two","complete")],
        "completed":true,"wireModel":"chirp-auk-turbo"}}),
        ),
        Reply::audio(),
        Reply::audio(),
    ]
}
async fn browser_fixture(peer: &Server, mode: u8) -> Fixture {
    let fixture = Box::pin(Fixture::with_client(peer, |client| {
        client.browser_executor_base_url = Some(peer.origin.clone());
        client.browser_executor_bearer_token = Some("synthetic-executor".into());
        client.request_time_browser_policy =
            crate::upstream::request_time_browser_policy::RequestTimeBrowserPolicy::LocalAllowed;
    }))
    .await;
    let mut document =
        serde_json::to_value(fixture.state.route_config.snapshot().document()).unwrap();
    match mode {
        0 => document["providers"][0]["execution_mode"] = json!("browser_backed"),
        1 => {
            document["providers"][0]["endpoint_execution_modes"] =
                json!({"music_generations":"browser_backed"})
        }
        2 => document["providers"][0]["credentials"][1]["execution_mode"] = json!("browser_backed"),
        _ => unreachable!(),
    }
    // The normal UI validates the real product audience; all outbound work still goes to
    // our explicit non-forwarding executor. Never weaken the production target check.
    document["providers"][0]["base_url"] = json!("https://studio-api-prod.suno.com");
    fixture
        .state
        .route_config
        .replace_document(serde_json::from_value(document).unwrap())
        .unwrap();
    fixture
}
async fn run(
    fixture: &Fixture,
    input: Value,
) -> crate::provider_runtime::ProviderPayloadProbeReport {
    let target = fixture
        .state
        .route_config
        .snapshot()
        .select_credential_probe_target("exact")
        .unwrap();
    let request = ConsoleProbeRequest::parse(input.to_string().as_bytes())
        .unwrap()
        .unwrap();
    Box::pin(probe_console_target_with_request(
        &fixture.state,
        &target,
        Some(&request),
    ))
    .await
    .1
}
fn wire(peer: &Server, assets: usize) {
    let calls = peer.calls();
    assert_eq!(
        calls.len(),
        1 + assets,
        "no direct generation, legacy endpoint or replay"
    );
    assert_eq!(
        calls[0].path,
        "/v1/internal/browser-executor/suno-bound-v1/execute"
    );
    assert_eq!(
        calls[0].headers["authorization"],
        "Bearer synthetic-executor"
    );
    let input = &calls[0].body["input"];
    assert_eq!(input["baseUrl"], "https://studio-api-prod.suno.com");
    assert_eq!(
        input["requestBody"],
        json!({"model":"chirp-auk-turbo","prompt":"合成音乐测试","waitCompletion":true})
    );
    assert_eq!(input["cookieHeader"], format!("__session={SESSION}"));
    assert_eq!(input["waitCompletion"], true);
    for call in &calls[1..] {
        assert_eq!(call.method, axum::http::Method::GET);
        for name in [
            "cookie",
            "authorization",
            "origin",
            "referer",
            "device-id",
            "browser-token",
        ] {
            assert!(!call.headers.contains_key(name), "asset received {name}");
        }
    }
}
fn audit(row: &crate::db::GatewayRequestAuditView, verified: bool) {
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
            json!({"verified":true,"count":2,"wireModel":"chirp-auk-turbo","supplierCharge":null})
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
        "synthetic-executor",
        "private",
    ] {
        assert!(!raw.contains(private), "audit leaked {private}");
    }
}

#[tokio::test]
async fn suno_probe_browser_bound_music_uses_exact_alias_account_and_complete_audio() {
    Box::pin(async {
        for mode in 0..3 {
            let peer = Server::start(completed()).await;
            let fixture = browser_fixture(&peer, mode).await;
            let before =
                serde_json::to_value(fixture.state.route_config.snapshot().document()).unwrap();
            let report =
                crate::upstream::public_assets::within(&peer.origin, run(&fixture, options()))
                    .await;
            assert_eq!(
                report.status,
                ProviderPayloadProbeStatus::Passed,
                "{}",
                report.message
            );
            assert!(report.answer.is_none() && report.health_recovery.is_some());
            assert!(
                report.message.contains("已验证 2 首 MP3") && report.message.contains("扣额未知")
            );
            let row = fixture.audit().await;
            assert_eq!(row.status, "completed");
            audit(&row, true);
            wire(&peer, 2);
            assert_eq!(fixture.balance().await.remaining_messages, Some(2));
            assert_eq!(
                serde_json::to_value(fixture.state.route_config.snapshot().document()).unwrap(),
                before
            );
            fixture.finish().await;
            peer.stop().await;
        }
    })
    .await;
}
