//! Management grant, exact account and persisted plan results use the dedicated sender.
use super::*;
use axum::http::Method;
use support::{grant_console_secret_access, parse_json, send_console_json, ConsoleStateFixture};
const PATH: &str = "/v1/internal/gateway/console/providers/arbitrary/probe";
fn policy() -> Value {
    json!({"automaticEnabled":false,"intervalMinutes":15,"modelSelection":"selected","models":["deployment1"],
        "cases":[{"id":"text","name":"Text","prompt":"hello","difficulty":1,"enabled":true}]})
}
fn run<F: std::future::Future<Output = ()> + 'static>(f: fn() -> F) {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(f())
        })
        .unwrap()
        .join()
        .unwrap();
}
#[test]
fn sap_aicore_console_probe_management_grant_and_all_plan_sources() {
    run(plans);
}
async fn plans() {
    let peer = peer::Peer::new("chat", false, false, 200).await;
    let mut doc = super::super::tests::document();
    doc["providers"][0]["credentials"][0]["test_policy"] = policy();
    doc["providers"][0]["test_plans"] = json!([{"id":"named","name":"Named","scopes":[{"kind":"account","id":"stable"}],"policy":policy()}]);
    let mut f = ConsoleStateFixture::new(serde_json::from_value(doc).unwrap(), true);
    let root = std::env::temp_dir().join(format!("sap-probe-{}", uuid::Uuid::new_v4()));
    let local = crate::local_runtime::LocalRuntime::open(&root)
        .await
        .unwrap();
    let state = Arc::get_mut(&mut f.state).unwrap();
    state.upstream_client.http = peer.client();
    state.local_runtime = Some(local.clone());
    let grant = grant_console_secret_access(&f.state).await;
    for (body, grant, expected) in [
        (
            json!({"model":"deployment1","prompt":"hello","credentialIds":["stable"]}),
            None,
            StatusCode::FORBIDDEN,
        ),
        (
            json!({"model":"deployment1","prompt":"hello","credentialIds":["foreign"]}),
            Some(grant.as_str()),
            StatusCode::BAD_REQUEST,
        ),
    ] {
        let response = send_console_json(&f.state, Method::POST, PATH, body, grant, None).await;
        assert_eq!(response.status(), expected);
    }
    assert!(peer.oauth.lock().unwrap().is_empty());
    assert!(peer.calls.lock().unwrap().is_empty());
    super::super::test_support::origin(peer.origin.clone(),async {
        for body in [
            json!({"model":"deployment1","prompt":"hello","credentialIds":["stable"]}),
            json!({"testPlan":policy(),"scope":{"kind":"account","id":"stable"},"credentialIds":["stable"]}),
            json!({"planId":"named","credentialIds":["stable"]}),
        ] {
            let response=send_console_json(&f.state,Method::POST,PATH,body,Some(&grant),None).await;
            let status=response.status();
            let body=parse_json(response).await;
            assert_eq!(status,StatusCode::OK,"{body}");
            assert_eq!(body["result"]["results"][0]["status"],"passed","{body}");
            assert!(!body.to_string().contains("synthetic-sap-secret"));
        }
        let response=support::probe_credential(&f.state,&grant,"stable").await;
        let status=response.status();
        let body=parse_json(response).await;
        assert_eq!(status,StatusCode::OK,"{body}");
        assert_eq!(body["result"]["status"],"passed","{body}");
        assert_eq!(body["result"]["assessment"]["policySource"],"account","{body}");
    }).await;
    assert_wire(&peer, 4, "chat/completions");
    let rows = local
        .list_audits(&db::RequestAuditFilters::default())
        .await
        .unwrap();
    assert_eq!(rows.len(), 4);
    assert!(rows
        .iter()
        .all(|r| r.status == "completed" && r.total_tokens == Some(5)));
    assert!(rows
        .iter()
        .all(|r| r.route_trace.as_ref().unwrap()["realCredentialRef"] == "stable"));
    let results = parse_json(
        send_console_json(
            &f.state,
            Method::POST,
            &format!("{PATH}/results"),
            json!({"includePlans":true}),
            Some(&grant),
            None,
        )
        .await,
    )
    .await;
    assert!(!results.to_string().contains("synthetic-sap-secret"));
    assert!(results.to_string().contains("named"), "{results}");
    peer.stop().await;
    drop(f);
    local.close().await;
}
#[test]
fn sap_aicore_console_probe_disabled_and_wrong_model_do_not_send() {
    run(rejected);
}
async fn rejected() {
    let peer = peer::Peer::new("chat", false, false, 200).await;
    let mut doc = super::super::tests::document();
    let mut disabled = doc["providers"][0]["credentials"][0].clone();
    disabled["id"] = json!("disabled");
    disabled["enabled"] = json!(false);
    doc["providers"][0]["credentials"]
        .as_array_mut()
        .unwrap()
        .push(disabled);
    let mut f = ConsoleStateFixture::new(serde_json::from_value(doc).unwrap(), true);
    Arc::get_mut(&mut f.state).unwrap().upstream_client.http = peer.client();
    let grant = grant_console_secret_access(&f.state).await;
    for (account, model) in [("disabled", "deployment1"), ("stable", "other")] {
        let response = send_console_json(
            &f.state,
            Method::POST,
            PATH,
            json!({"model":model,"prompt":"hello","credentialIds":[account]}),
            Some(&grant),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = parse_json(response).await;
        assert_eq!(
            body["result"]["results"][0]["status"], "unsupported",
            "{body}"
        );
    }
    assert!(peer.oauth.lock().unwrap().is_empty());
    assert!(peer.calls.lock().unwrap().is_empty());
    peer.stop().await;
}
