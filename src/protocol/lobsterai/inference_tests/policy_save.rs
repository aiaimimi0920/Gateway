//! 实际管理保存策略后调度生效，关闭后持久保留；不直接替换内存路由。
use super::super::test_support::{Peer, Reply};
use super::*;
use crate::console::secrets::{redact_route_document, SecretPatch};
use crate::provider_credential_probe_scheduler::sweep_scheduled_provider_credentials_once as sweep;
use crate::provider_runtime::test_store;
use axum::http::Method;
use support::{grant_console_secret_access, parse_json, send_console_json};

fn policy(enabled: bool) -> Value {
    json!({"automaticEnabled":enabled,"intervalMinutes":1,"modelSelection":"selected","models":[MODEL],
        "cases":[{"id":"saved","name":"Saved","prompt":"saved through management API",
            "expectedAnswer":"合成答案","difficulty":1,"enabled":true}]})
}
fn commit_body(f: &Fixture, enabled: bool) -> Value {
    let current = f.state.route_config.snapshot();
    let redacted = redact_route_document(current.document()).unwrap();
    let patches: Vec<_> = redacted
        .secrets
        .into_iter()
        .filter(|s| s.configured)
        .map(|s| SecretPatch::keep(s.path))
        .collect();
    let mut document = serde_json::to_value(redacted.document).unwrap();
    document["providers"][0]["credentials"][0]["test_policy"] = policy(enabled);
    document["providers"][0]["test_plans"] = json!([{"id":"saved-plan","name":"Saved plan",
        "scopes":[{"kind":"account","id":"exact"}],"policy":policy(enabled)}]);
    json!({"expectedRevision":current.revision().id(),"document":document,"secretPatches":patches})
}
async fn commit(f: &Fixture, grant: &str, body: Value, expected: StatusCode) {
    let response = Box::pin(send_console_json(
        &f.state,
        Method::PUT,
        "/v1/internal/gateway/console/route-config",
        body,
        Some(grant),
        None,
    ))
    .await;
    let status = response.status();
    let body = parse_json(response).await;
    assert_eq!(status, expected, "{body}");
    assert!(!body.to_string().contains(KEY));
    assert!(!body.to_string().contains("synthetic-refresh"));
    if expected == StatusCode::OK {
        assert_eq!(body["committed"], true);
    }
}
fn credential_material(f: &Fixture) -> Value {
    let document = serde_json::to_value(f.state.route_config.snapshot().document()).unwrap();
    let credential = &document["providers"][0]["credentials"][0];
    json!({"id":credential["id"],"api_key":credential["api_key"],"refresh_token":credential["refresh_token"],
        "lobsterai":credential["lobsterai"],"lobsterai_catalog":credential["lobsterai_catalog"]})
}
#[tokio::test]
async fn lobsterai_policy_save_management_commit_drives_scheduler_and_persists_disabled_state() {
    Box::pin(async {
        let peer = Peer::start(
            (0..2)
                .map(|_| Reply::raw(200, native::reply("chat", false, false, true)))
                .collect(),
        )
        .await;
        let f = Box::pin(Fixture::with_http(
            peer.builder()
                .retry(rquest::retry::Policy::never())
                .build()
                .unwrap(),
            peer.origin.clone(),
        ))
        .await;
        let material = credential_material(&f);
        let original_revision = f.state.route_config.snapshot().revision().id().to_owned();
        let grant = grant_console_secret_access(&f.state).await;
        let body = commit_body(&f, true);
        commit(&f, &grant, body.clone(), StatusCode::OK).await;
        assert_ne!(
            f.state.route_config.snapshot().revision().id(),
            original_revision
        );
        assert_eq!(credential_material(&f), material);
        let committed = std::fs::read(&f.state.config.console.routes_file).unwrap();
        commit(&f, &grant, body, StatusCode::CONFLICT).await;
        assert_eq!(
            std::fs::read(&f.state.config.console.routes_file).unwrap(),
            committed
        );
        peer.inspect(|calls| assert!(calls.is_empty(), "saving configuration must not generate"));
        let summary = super::super::transport::TEST_ORIGIN
            .scope(peer.origin.clone(), Box::pin(sweep(&f.state)))
            .await
            .unwrap();
        assert_eq!(
            (
                summary.scheduled_count,
                summary.due_count,
                summary.passed_count
            ),
            (2, 2, 2)
        );
        for plan in [None, Some("saved-plan")] {
            let results = test_store::read_many(&f.state, "lobsterai-public", &["exact"], plan)
                .await
                .unwrap();
            assert_eq!(results.len(), 1);
            let assessment = results[0].assessment.as_ref().unwrap();
            assert_eq!(assessment.mode, "automatic");
            assert_eq!(assessment.plan_id.as_deref(), plan);
            assert_eq!(assessment.cases[0].correct, Some(true));
        }
        commit(&f, &grant, commit_body(&f, false), StatusCode::OK).await;
        assert_eq!(sweep(&f.state).await.unwrap().scheduled_count, 0);
        assert_eq!(credential_material(&f), material);
        let f = Box::pin(f.restart(peer.origin.clone())).await;
        assert_eq!(sweep(&f.state).await.unwrap().scheduled_count, 0);
        assert_eq!(credential_material(&f), material);
        assert_eq!(f.remaining().await, 2);
        let audits = f.audits(2).await;
        assert!(audits
            .iter()
            .all(|r| r.status == "completed" && r.total_tokens == Some(5)));
        peer.inspect(|calls| {
            assert_eq!(calls.len(), 2);
            for call in calls {
                assert_eq!(call.path, "/api/proxy/v1/chat/completions");
                assert_eq!(call.headers["authorization"], format!("Bearer {KEY}"));
                assert_eq!(call.body["model"], UPSTREAM_MODEL);
                assert_eq!(
                    call.body["messages"][0]["content"],
                    "saved through management API"
                );
            }
        });
        f.finish().await;
    })
    .await;
}
