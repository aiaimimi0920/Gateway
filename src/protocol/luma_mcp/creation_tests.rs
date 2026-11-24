//! Normal management path -> SQLite audit -> synthetic MCP; never calls a live provider.
use super::{
    browser_fixture, creation,
    fixture::{Peer, Reply},
    test_support, VERSION,
};
use axum::http::Method;
use serde_json::{json, Value};
use std::sync::Arc;
#[path = "creation_browser_test.rs"]
mod browser_test;
#[path = "creation_model_tests.rs"]
mod model_tests;
#[path = "creation_options_tests.rs"]
mod options_tests;
#[path = "creation_recovery_tests.rs"]
mod recovery_tests;
#[path = "creation_reuse_tests.rs"]
mod reuse_tests;
#[path = "creation_review_tests.rs"]
mod review_tests;
#[path = "creation_server_tests.rs"]
mod server_tests;
#[path = "../../../tests/console_contract_support/mod.rs"]
mod support;

fn replies(values: Vec<Value>) -> Vec<Reply> {
    values.into_iter().flat_map(|value| {
        let mut init = Reply::json(json!({"jsonrpc":"2.0","id":1,"result":{"protocolVersion":VERSION,"capabilities":{"tools":{}}}}));
        init.session = Some("transport-session");
        [init, Reply::initialized(), Reply::json(json!({"jsonrpc":"2.0","id":2,"result":{"structuredContent":value}}))]
    }).collect()
}
fn opened() -> Value {
    json!({"session_id":"session:s1","workspace_id":"w1","state":"active","replayed":false,"expires_at":"later","resource_uri":"luma://session/s1"})
}
fn started() -> Value {
    json!({"run_id":"run:r1","kind":"generate","state":"running"})
}
fn progress(state: &str, timeout: bool, outputs: Value) -> Value {
    json!({"session_id":"s1","run_id":"r1","kind":"generate","status":state,"timed_out":timeout,"outputs":outputs})
}
fn usage(estimated: bool) -> Value {
    json!({"usage":{"session_id":"s1","total_credits":9,"unresolved_event_count":0,
        "jobs":[{"job_id":"j1","credits":9,"credits_estimated":estimated}]}})
}
struct Fixture {
    root: std::path::PathBuf,
    state: Arc<crate::state::AppState>,
    grant: String,
    peer: Peer,
}
impl Fixture {
    async fn new(values: Vec<Value>) -> Self {
        Self::with_replies(replies(values)).await
    }
    async fn with_replies(replies: Vec<Reply>) -> Self {
        let root = std::env::temp_dir().join(format!(
            "gateway-local-luma-mcp-creation-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let state = browser_fixture::state(&root).await;
        let grant = support::grant_console_secret_access(&state).await;
        Self {
            root,
            state,
            grant,
            peer: Peer::new(replies).await,
        }
    }
    fn command(&self, action: &str, id: Option<&str>) -> Value {
        let mut body = json!({"providerId":"luma-mcp","credentialId":"saved-mcp", "expectedRevision":self.state.route_config.snapshot().revision().id(),"action":action});
        if let Some(id) = id {
            body["taskId"] = json!(id);
        }
        if matches!(action, "prepare" | "start") {
            body["request"] = json!({"kind":"image","prompt":"a blue circle","budgetCredits":20});
        }
        if action == "start" {
            body["chargeConfirmed"] = json!(true);
        }
        body
    }
    async fn send(&self, body: Value) -> (u16, Value) {
        let response = test_support::scope(
            self.peer.endpoint.clone(),
            support::send_console_json(
                &self.state,
                Method::POST,
                "/v1/internal/gateway/console/luma-mcp-creation",
                body,
                Some(&self.grant),
                None,
            ),
        )
        .await;
        (
            response.status().as_u16(),
            support::parse_json(response).await,
        )
    }
    async fn act(&self, action: &str, id: Option<&str>) -> Value {
        let (status, body) = self.send(self.command(action, id)).await;
        assert_eq!(status, 200, "{body}");
        body["task"].clone()
    }
    async fn close(self) {
        self.peer.stop().await;
        browser_fixture::close(&self.state).await;
        crate::local_runtime::test_support::remove_test_root(&self.root).await;
    }
}

#[tokio::test]
async fn luma_mcp_creation_normal_path_persists_and_never_resubmits() {
    let output = json!([{"asset_id":"artifact:a1","type":"image","model":null}]);
    let f = Fixture::new(vec![opened(),started(),progress("running",true,json!([])),usage(true),
        progress("completed",false,output),json!({"assets":[{"asset_id":"a1","type":"image","has_image":true,"state":"ready","url":"https://cdn.example.com/result.png"}]}),usage(false),
        json!({"status":"ended","session_id":"s1"}),usage(false)]).await;
    let task = f.act("prepare", None).await;
    let id = task["id"].as_str().unwrap();
    assert!(f.peer.calls.lock().unwrap().is_empty());
    let mut denied = f.command("start", Some(id));
    denied["chargeConfirmed"] = json!(false);
    assert_eq!(f.send(denied).await.0, 400);
    let start = f.act("start", Some(id)).await;
    assert_eq!(start["state"], "running");
    assert_eq!(start["runId"], "r1");
    assert_eq!(f.send(f.command("start", Some(id))).await.0, 409);
    let wait = f.act("read", Some(id)).await;
    assert_eq!(wait["state"], "running");
    assert_eq!(wait["usage"]["settled"], false);
    let done = f.act("read", Some(id)).await;
    assert_eq!(done["state"], "completed");
    assert_eq!(
        done["assets"][0]["url"],
        "https://cdn.example.com/result.png"
    );
    assert_eq!(done["usage"]["credits"], 9);
    assert_eq!(done["usage"]["settled"], true);
    let mut other = f.command("read", Some(id));
    other["credentialId"] = json!("other-mcp");
    assert_eq!(f.send(other).await.0, 400);
    assert_eq!(f.act("end", Some(id)).await["ended"], true);
    let row: String = sqlx::query_scalar("SELECT payload FROM request_audits WHERE id = ?")
        .bind(id)
        .fetch_one(&f.state.local_runtime.as_ref().unwrap().pool)
        .await
        .unwrap();
    assert!(row.contains("lumaMcpCreation"));
    assert!(!row.contains("a blue circle"));
    {
        let calls = f.peer.calls.lock().unwrap();
        let tools: Vec<_> = calls
            .iter()
            .filter(|(_, v)| v["method"] == "tools/call")
            .collect();
        assert_eq!(
            tools
                .iter()
                .filter(|(_, v)| v["params"]["name"] == "luma_generate_image")
                .count(),
            1
        );
        assert_eq!(tools[0].1["params"]["arguments"]["workspace_key"], id);
        for (headers, _) in calls.iter() {
            assert_eq!(headers["authorization"], "Bearer synthetic-access");
            assert!(!headers.contains_key("x-secret-grant"));
            assert!(!headers.contains_key("cookie"));
        }
    }
    f.close().await;
}

#[tokio::test]
async fn luma_mcp_creation_rejects_foreign_run_and_cancels_original() {
    let mut foreign = progress("completed", false, json!([]));
    foreign["session_id"] = json!("other");
    let f = Fixture::new(vec![
        opened(),
        started(),
        foreign,
        json!({"status":"cancelled","run_id":"r1","session_id":"s1"}),
        usage(true),
    ])
    .await;
    let task = f.act("prepare", None).await;
    let id = task["id"].as_str().unwrap();
    f.act("start", Some(id)).await;
    let invalid = f.act("read", Some(id)).await;
    assert_eq!(invalid["state"], "running");
    assert_eq!(invalid["error"], "luma_mcp_invalid_response");
    let cancelled = f.act("cancel", Some(id)).await;
    assert_eq!(cancelled["state"], "cancelled");
    assert_eq!(cancelled["ended"], false);
    let calls = f.peer.calls.lock().unwrap().clone();
    assert!(calls
        .iter()
        .any(|(_, v)| v["params"]["name"] == "luma_session_cancel"
            && v["params"]["arguments"] == json!({"run_id":"r1"})));
    f.close().await;
}

#[test]
fn luma_mcp_creation_pending_unknown_cost_and_asset_boundaries() {
    let mut pending = progress("completed", false, json!([]));
    pending["pending"] = json!({"tool":"untrusted"});
    assert_eq!(
        creation::progress(&pending, "s1", "r1", creation::Kind::Image).unwrap()["state"],
        "needs_input"
    );
    pending["pending"] = Value::Null;
    pending["settled_with"] = json!("run:r2");
    assert_eq!(
        creation::progress(&pending, "s1", "r1", creation::Kind::Image).unwrap()["state"],
        "superseded"
    );
    assert!(creation::usage(
        &json!({"usage":{"session_id":"other","total_credits":0,"unresolved_event_count":0}}),
        "s1"
    )
    .is_err());
    assert!(creation::usage(
        &json!({"usage":{"session_id":"s1","total_credits":null,"unresolved_event_count":1}}),
        "s1"
    )
    .is_err());
    let row = |url: &str| json!({"assets":[{"asset_id":"a1","type":"image","has_image":true,"state":"ready","url":url}]});
    for url in [
        "http://example.com/a",
        "https://127.0.0.1/a",
        "https://user:secret@example.com/a",
    ] {
        assert!(creation::assets(&row(url), &["a1".into()], creation::Kind::Image).is_err());
    }
    assert!(creation::assets(
        &row("https://example.com/a"),
        &["other".into()],
        creation::Kind::Image
    )
    .is_err());
}

#[tokio::test]
async fn luma_mcp_creation_lost_submission_is_not_replayed() {
    let f = Fixture::new(vec![opened(), json!({"private":"supplier-secret"})]).await;
    let task = f.act("prepare", None).await;
    let id = task["id"].as_str().unwrap();
    let result = f.act("start", Some(id)).await;
    assert_eq!(result["state"], "unconfirmed");
    assert_eq!(result["sessionId"], "s1");
    assert!(result["runId"].is_null());
    assert!(!result.to_string().contains("supplier-secret"));
    let before = f.peer.calls.lock().unwrap().len();
    assert_eq!(f.send(f.command("start", Some(id))).await.0, 409);
    assert_eq!(f.peer.calls.lock().unwrap().len(), before);
    f.close().await;
}

#[tokio::test]
async fn luma_mcp_creation_disconnect_preserves_video_receipt_and_restart_lookup() {
    let mut responses = replies(vec![
        opened(),
        started(),
        progress("running", true, json!([])),
        usage(true),
    ]);
    responses[5].delay = std::time::Duration::from_millis(350);
    let mut f = Fixture::with_replies(responses).await;
    let mut prepare = f.command("prepare", None);
    prepare["request"]["kind"] = json!("video");
    let (status, task) = f.send(prepare).await;
    assert_eq!(status, 200);
    let id = task["task"]["id"].as_str().unwrap().to_owned();
    let mut start = f.command("start", Some(&id));
    start["request"]["kind"] = json!("video");
    let (state, grant, endpoint) = (f.state.clone(), f.grant.clone(), f.peer.endpoint.clone());
    let waiter = tokio::spawn(async move {
        test_support::scope(
            endpoint,
            support::send_console_json(
                &state,
                Method::POST,
                "/v1/internal/gateway/console/luma-mcp-creation",
                start,
                Some(&grant),
                None,
            ),
        )
        .await
    });
    f.peer.wait_calls(6).await;
    waiter.abort();
    let _ = waiter.await;
    // Drain waits for the already admitted operation, even though its HTTP waiter disappeared.
    browser_fixture::close(&f.state).await;
    f.state = browser_fixture::state(&f.root).await;
    f.grant = support::grant_console_secret_access(&f.state).await;
    let read = f.act("read", Some(&id)).await;
    assert_eq!(read["runId"], "r1");
    assert_eq!(read["state"], "running");
    let calls = f.peer.calls.lock().unwrap().clone();
    let generates: Vec<_> = calls
        .iter()
        .filter(|(_, v)| v["params"]["name"] == "luma_generate_video")
        .collect();
    assert_eq!(generates.len(), 1);
    assert_eq!(
        generates[0].1["params"]["arguments"]["model"],
        "luma_ray3_2"
    );
    f.close().await;
}
