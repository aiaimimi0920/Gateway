//! Management grant, exact account and persisted plan results use the dedicated sender.
use super::*;
use axum::http::Method;
use support::{grant_console_secret_access, parse_json, send_console_json, ConsoleStateFixture};
const PATH: &str = "/v1/internal/gateway/console/providers/arbitrary/probe";
fn policy() -> Value {
    json!({"automaticEnabled":false,"intervalMinutes":15,"modelSelection":"selected","models":["MiniMax-M3.1-Flash"],
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
fn minimax_console_probe_management_grant_and_all_plan_sources() {
    run(plans);
}
async fn plans() {
    for preset in [super::super::PRESET, super::super::PRESET_CN] {
        plans_region(preset).await;
    }
}
async fn plans_region(preset: &str) {
    let peer = peer::Peer::new("messages", false, false, 200).await;
    let mut doc = document();
    doc["providers"][0]["preset"] = json!(preset);
    doc["providers"][0]["base_url"] = json!(super::super::preset_base(preset).unwrap());
    doc["providers"][0]["credentials"][0]["test_policy"] = policy();
    doc["providers"][0]["test_plans"] = json!([{"id":"named","name":"Named","scopes":[{"kind":"account","id":"stable"}],"policy":policy()}]);
    let mut f = ConsoleStateFixture::new(serde_json::from_value(doc).unwrap(), true);
    let root = std::env::temp_dir().join(format!("minimax-probe-{}", uuid::Uuid::new_v4()));
    let local = crate::local_runtime::LocalRuntime::open(&root)
        .await
        .unwrap();
    let state = Arc::get_mut(&mut f.state).unwrap();
    state.upstream_client.http = peer.client();
    state.local_runtime = Some(local.clone());
    let grant = grant_console_secret_access(&f.state).await;
    for (body, grant, expected) in [
        (
            json!({"model":"MiniMax-M3.1-Flash","prompt":"hello","credentialIds":["stable"]}),
            None,
            StatusCode::FORBIDDEN,
        ),
        (
            json!({"model":"MiniMax-M3.1-Flash","prompt":"hello","credentialIds":["foreign"]}),
            Some(grant.as_str()),
            StatusCode::BAD_REQUEST,
        ),
    ] {
        let response = send_console_json(&f.state, Method::POST, PATH, body, grant, None).await;
        assert_eq!(response.status(), expected);
    }
    assert!(peer.calls.lock().unwrap().is_empty());
    super::super::test_support::origin(peer.origin.clone(),async {
        for body in [
            json!({"model":"MiniMax-M3.1-Flash","prompt":"hello","credentialIds":["stable"]}),
            json!({"testPlan":policy(),"scope":{"kind":"account","id":"stable"},"credentialIds":["stable"]}),
            json!({"planId":"named","credentialIds":["stable"]}),
        ] {
            let response=send_console_json(&f.state,Method::POST,PATH,body,Some(&grant),None).await;
            let status=response.status();
            let body=parse_json(response).await;
            assert_eq!(status,StatusCode::OK,"{body}");
            assert_eq!(body["result"]["results"][0]["status"],"passed","{body}");
            assert!(!body.to_string().contains("synthetic-minimax"));
        }
        let response=support::probe_credential(&f.state,&grant,"stable").await;
        let status=response.status();
        let body=parse_json(response).await;
        assert_eq!(status,StatusCode::OK,"{body}");
        assert_eq!(body["result"]["status"],"passed","{body}");
        assert_eq!(body["result"]["assessment"]["policySource"],"account","{body}");
    }).await;
    let calls = peer.calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 4);
    for (path, headers, body) in calls {
        assert_eq!(path, "/anthropic/v1/messages");
        assert_eq!(headers["authorization"], "Bearer synthetic-minimax");
        assert_eq!(body["model"], UPSTREAM_MODEL);
    }
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
    assert!(!results.to_string().contains("synthetic-minimax"));
    assert!(results.to_string().contains("named"), "{results}");
    peer.stop().await;
    drop(f);
    local.close().await;
}
#[test]
fn minimax_console_probe_disabled_and_wrong_model_do_not_send() {
    run(rejected);
}
async fn rejected() {
    let peer = peer::Peer::new("messages", false, false, 200).await;
    let mut doc = document();
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
    let response = support::probe_credential(&f.state, &grant, "stable").await;
    let body = parse_json(response).await;
    assert_eq!(body["result"]["status"], "unsupported", "{body}");
    for (account, model) in [("disabled", "MiniMax-M3.1-Flash"), ("stable", "other")] {
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
    assert!(peer.calls.lock().unwrap().is_empty());
    peer.stop().await;
}

fn document() -> Value {
    json!({"providers":[{"id":"arbitrary","preset":"minimax-m-plan","base_url":super::super::BASE,
        "explicit_credentials_only":true,"credentials":[{"id":"stable","api_key":"synthetic-minimax",
        "supported_models":[UPSTREAM_MODEL]}]}]})
}
