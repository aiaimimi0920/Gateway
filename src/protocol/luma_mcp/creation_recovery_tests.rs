//! Recovery must prove account continuity before any saved-task read, cleanup or workspace reuse.
use super::*;
#[path = "creation_recovery_fixture.rs"]
mod fixture;
use fixture::{commit, offline, refresh, restart, unpublished_successor};

#[tokio::test]
async fn luma_mcp_creation_recovery_policy_edit_keeps_old_task_and_current_revision_fence() {
    let f = Fixture::new(vec![
        opened(),
        started(),
        progress("running", true, json!([])),
        usage(true),
        json!({"status":"cancelled","run_id":"r1","session_id":"s1"}),
        usage(false),
        json!({"status":"ended","session_id":"s1"}),
        usage(false),
    ])
    .await;
    let task = f.act("prepare", None).await;
    let id = task["id"].as_str().unwrap();
    let stale = f.command("start", Some(id));
    commit(&f, |d| {
        d.providers[0].credentials[1].account_name = Some("unrelated name".into())
    })
    .await;
    assert_eq!(f.send(stale).await.0, 409);
    assert!(f.peer.calls.lock().unwrap().is_empty());
    let running = f.act("start", Some(id)).await;
    assert_eq!(running["revision"], task["revision"]);
    assert_eq!(running["state"], "running");
    assert_eq!(f.send(f.command("start", Some(id))).await.0, 409);
    assert_eq!(f.act("read", Some(id)).await["runId"], "r1");
    assert_eq!(f.act("cancel", Some(id)).await["state"], "cancelled");
    assert_eq!(f.act("end", Some(id)).await["ended"], true);
    assert_eq!(
        f.peer
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, v)| v["params"]["name"] == "luma_generate_image")
            .count(),
        1
    );
    f.close().await;
}

#[tokio::test]
async fn luma_mcp_creation_recovery_published_rotations_restart_and_workspace_reuse() {
    let mut values = reuse_tests::source_replies("image");
    values.extend([
        opened(),
        started(),
        json!({"status":"ended","session_id":"s1"}),
        usage(false),
    ]);
    let mut f = Fixture::new(values).await;
    offline(&f).await;
    let task = f.act("prepare", None).await;
    let id = task["id"].as_str().unwrap().to_owned();
    f.act("start", Some(&id)).await;
    refresh(&f, 1).await;
    commit(&f, |d| {
        d.providers[0].credentials[0].account_name = Some("same account".into())
    })
    .await;
    refresh(&f, 2).await;
    restart(&mut f).await;
    let before = f.peer.calls.lock().unwrap().len();
    let done = f.act("read", Some(&id)).await;
    assert_eq!(done["state"], "completed");
    assert_eq!(done["revision"], task["revision"]);
    assert_eq!(done["usage"]["credits"], 9);
    let request = reuse_tests::input("image", &id, true);
    let child = reuse_tests::prepare(&f, &request).await;
    let next = reuse_tests::start(&f, &child, &request).await;
    assert_eq!(next["workspaceBinding"]["key"], id);
    assert_ne!(next["revision"], task["revision"]);
    assert_eq!(f.act("end", Some(&id)).await["ended"], true);
    let calls = f.peer.calls.lock().unwrap().clone();
    assert!(calls[before..]
        .iter()
        .all(|(headers, _)| headers["authorization"] == "Bearer rotation-2"));
    assert_eq!(
        calls
            .iter()
            .filter(|(_, v)| v["params"]["name"] == "luma_generate_image")
            .count(),
        2
    );
    f.close().await;
}

#[tokio::test]
async fn luma_mcp_creation_recovery_rejects_replaced_account_and_missing_archive_without_dispatch()
{
    let f = Fixture::new(reuse_tests::source_replies("image")).await;
    let id = reuse_tests::source(&f, "image").await;
    let before = f.peer.calls.lock().unwrap().len();
    // Same credential ID and OAuth client are not evidence of the same account.
    commit(&f, |d| {
        d.providers[0].credentials[0].api_key = Some("different-account".into())
    })
    .await;
    for action in ["read", "cancel", "end", "start"] {
        let (status, body) = f.send(f.command(action, Some(&id))).await;
        assert_eq!(status, 409, "{body}");
        assert_eq!(body["error"]["code"], "luma_mcp_task_identity_unverified");
        assert!(!body.to_string().contains("different-account"));
    }
    let mut reuse = f.command("prepare", None);
    reuse["request"] = reuse_tests::input("image", &id, true);
    assert_eq!(f.send(reuse).await.0, 409);
    sqlx::query("UPDATE request_audits SET payload = json_set(payload, '$.routeTrace.lumaMcpCreation.revision', 'missing') WHERE id = ?")
        .bind(&id).execute(&f.state.local_runtime.as_ref().unwrap().pool).await.unwrap();
    assert_eq!(f.send(f.command("read", Some(&id))).await.0, 409);
    assert_eq!(f.peer.calls.lock().unwrap().len(), before);
    f.close().await;
}

#[tokio::test]
async fn luma_mcp_creation_recovery_requires_published_refresh_evidence() {
    let f = Fixture::new(vec![]).await;
    offline(&f).await;
    let task = f.act("prepare", None).await;
    unpublished_successor(&f).await;
    let (status, body) = f.send(f.command("read", task["id"].as_str())).await;
    assert_eq!(status, 409, "{body}");
    assert_eq!(body["error"]["code"], "luma_mcp_task_identity_unverified");
    assert!(f.peer.calls.lock().unwrap().is_empty());
    f.close().await;
}

#[tokio::test]
async fn luma_mcp_creation_recovery_unconfirmed_submission_never_replays_after_revision_change() {
    let mut f = Fixture::new(vec![
        opened(),
        json!({"invalid":"lost receipt"}),
        usage(true),
        json!({"status":"ended","session_id":"s1"}),
        usage(false),
    ])
    .await;
    let task = f.act("prepare", None).await;
    let id = task["id"].as_str().unwrap().to_owned();
    assert_eq!(f.act("start", Some(&id)).await["state"], "unconfirmed");
    commit(&f, |d| {
        d.providers[0].credentials[0].account_name = Some("renamed".into())
    })
    .await;
    restart(&mut f).await;
    assert_eq!(f.act("read", Some(&id)).await["state"], "unconfirmed");
    assert_eq!(f.send(f.command("start", Some(&id))).await.0, 409);
    assert_eq!(f.act("end", Some(&id)).await["ended"], true);
    assert_eq!(
        f.peer
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, v)| v["params"]["name"] == "luma_generate_image")
            .count(),
        1
    );
    f.close().await;
}

#[tokio::test]
async fn luma_mcp_creation_recovery_account_change_during_open_blocks_generation() {
    let mut responses = replies(vec![opened()]);
    responses[2].delay = std::time::Duration::from_millis(300);
    let f = Fixture::with_replies(responses).await;
    let task = f.act("prepare", None).await;
    let id = task["id"].as_str().unwrap();
    let operation = f.send(f.command("start", Some(id)));
    let change = async {
        f.peer.wait_calls(3).await;
        commit(&f, |d| {
            d.providers[0].credentials[0].api_key = Some("replacement-during-open".into())
        })
        .await;
    };
    let (result, ()) = tokio::join!(operation, change);
    assert_eq!(result.0, 409);
    assert_eq!(f.peer.calls.lock().unwrap().len(), 3);
    assert_eq!(f.send(f.command("read", Some(id))).await.0, 409);
    f.close().await;
}
