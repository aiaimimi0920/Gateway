//! Workspace reuse crosses the real management route, durable journal and fixed MCP wire.
use super::*;

pub(super) fn source_replies(kind: &str) -> Vec<Value> {
    vec![
        opened(),
        started(),
        progress(
            "completed",
            false,
            json!([{"asset_id":"a1","type":kind},{"asset_id":"a2","type":kind}]),
        ),
        json!({"assets":[
            {"asset_id":"a1","type":kind,"has_image":true,"state":"ready","url":"https://cdn.example.com/a1"},
            {"asset_id":"a2","type":kind,"has_image":true,"state":"ready","url":null}]}),
        usage(false),
    ]
}
pub(super) async fn source(f: &Fixture, kind: &str) -> String {
    let mut prepare = f.command("prepare", None);
    prepare["request"]["kind"] = json!(kind);
    let (status, body) = f.send(prepare).await;
    assert_eq!(status, 200, "{body}");
    let id = body["task"]["id"].as_str().unwrap();
    let mut start = f.command("start", Some(id));
    start["request"]["kind"] = json!(kind);
    assert_eq!(f.send(start).await.0, 200);
    assert_eq!(
        f.act("read", Some(id)).await["assets"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    id.into()
}
pub(super) fn input(kind: &str, source: &str, gpt: bool) -> Value {
    json!({"kind":kind,"prompt":"continue with this asset","budgetCredits":13,
        "options":if gpt { json!({"model":"openai_gpt_image_2"}) } else { json!({"operation":"edit"}) },
        "reuse":{"taskId":source,"sourceAssetId":if gpt {Value::Null} else {json!("artifact:a1")},
            "referenceAssetIds":if gpt || kind == "image" {json!(["a2"])} else {json!([])}}})
}
pub(super) async fn prepare(f: &Fixture, request: &Value) -> String {
    let mut command = f.command("prepare", None);
    command["request"] = request.clone();
    let (status, body) = f.send(command).await;
    assert_eq!(status, 200, "{body}");
    body["task"]["id"].as_str().unwrap().into()
}
pub(super) async fn start(f: &Fixture, id: &str, request: &Value) -> Value {
    let mut command = f.command("start", Some(id));
    command["request"] = request.clone();
    let (status, body) = f.send(command).await;
    assert_eq!(status, 200, "{body}");
    body["task"].clone()
}

#[tokio::test]
async fn luma_mcp_creation_reuse_binds_images_references_and_video_without_url_fetch() {
    for (kind, gpt) in [("image", false), ("image", true), ("video", false)] {
        let mut values = source_replies(kind);
        let mut new_session = opened();
        new_session["session_id"] = json!("s2");
        values.extend([new_session, started()]);
        let f = Fixture::new(values).await;
        let source_id = source(&f, kind).await;
        let request = input(kind, &source_id, gpt);
        let before = f.peer.calls.lock().unwrap().len();
        let id = prepare(&f, &request).await;
        assert_eq!(f.peer.calls.lock().unwrap().len(), before);
        let mut tampered = f.command("start", Some(&id));
        tampered["request"] = request.clone();
        tampered["request"]["reuse"]["referenceAssetIds"] = json!([]);
        if kind == "video" {
            tampered["request"]["reuse"]["sourceAssetId"] = json!("a2");
        }
        assert_eq!(f.send(tampered).await.0, 400);
        let task = start(&f, &id, &request).await;
        assert_eq!(task["state"], "running");
        assert_eq!(task["workspaceBinding"]["key"], source_id);
        assert_eq!(task["budgetCredits"], 13);
        {
            let calls = f.peer.calls.lock().unwrap();
            let tools: Vec<_> = calls
                .iter()
                .filter(|(_, v)| v["method"] == "tools/call")
                .collect();
            let open = &tools[tools.len() - 2].1["params"]["arguments"];
            let generate = &tools[tools.len() - 1].1["params"]["arguments"];
            assert_eq!(open["workspace_key"], source_id);
            assert_eq!(open["budget_credits"], 13);
            assert_eq!(open["user_id"], id);
            assert_eq!(generate["session_id"], "s2");
            assert_eq!(generate["user_id"], id);
            if gpt {
                assert_eq!(
                    generate["medias"],
                    json!([{"asset_id":"a2","role":"reference"}])
                );
                assert!(generate.get("source").is_none());
            } else {
                assert_eq!(generate["source"], json!({"asset_id":"a1"}));
                if kind == "image" {
                    assert_eq!(generate["image_ref"], json!([{"asset_id":"a2"}]));
                }
            }
            assert!(!generate.to_string().contains("https://"));
        }
        f.close().await;
    }
}

#[tokio::test]
async fn luma_mcp_creation_reuse_survives_restart_and_keeps_root_workspace_for_descendants() {
    let mut values = source_replies("image");
    let mut second = source_replies("image");
    second[0]["session_id"] = json!("s2");
    second[2]["session_id"] = json!("s2");
    second[4]["usage"]["session_id"] = json!("s2");
    values.extend(second);
    values.extend([opened(), started()]);
    let mut f = Fixture::new(values).await;
    let root = source(&f, "image").await;
    // Historical journals predate workspaceBinding; their task ID was the workspace key.
    sqlx::query("UPDATE request_audits SET payload = json_remove(payload, '$.routeTrace.lumaMcpCreation.workspaceBinding') WHERE id = ?")
        .bind(&root).execute(&f.state.local_runtime.as_ref().unwrap().pool).await.unwrap();
    let request = input("image", &root, false);
    let id = prepare(&f, &request).await;
    browser_fixture::close(&f.state).await;
    f.state = browser_fixture::state(&f.root).await;
    f.grant = support::grant_console_secret_access(&f.state).await;
    start(&f, &id, &request).await;
    f.act("read", Some(&id)).await;
    let third = input("image", &id, true);
    let third_id = prepare(&f, &third).await;
    let result = start(&f, &third_id, &third).await;
    assert_eq!(result["workspaceBinding"]["key"], root);
    assert_eq!(result["workspaceBinding"]["taskId"], id);
    let calls = f.peer.calls.lock().unwrap().clone();
    let opens: Vec<_> = calls
        .iter()
        .filter(|(_, v)| v["params"]["name"] == "luma_session_open")
        .collect();
    assert_eq!(opens.len(), 3);
    assert!(opens
        .iter()
        .all(|(_, v)| v["params"]["arguments"]["workspace_key"] == root));
    assert_ne!(
        opens[0].1["params"]["arguments"]["idempotency_key"],
        opens[1].1["params"]["arguments"]["idempotency_key"]
    );
    f.close().await;
}

#[tokio::test]
async fn luma_mcp_creation_reuse_rejects_unknown_foreign_and_unready_assets_before_admission() {
    let f = Fixture::new(source_replies("image")).await;
    let id = source(&f, "image").await;
    let before = f.peer.calls.lock().unwrap().len();
    let local = f.state.local_runtime.as_ref().unwrap();
    for request in [
        input(
            "image",
            &format!("luma-mcp-{}", uuid::Uuid::new_v4()),
            false,
        ),
        input("video", &id, false),
        {
            let mut r = input("image", &id, false);
            r["reuse"]["sourceAssetId"] = json!("foreign");
            r
        },
    ] {
        let mut command = f.command("prepare", None);
        command["request"] = request;
        assert_eq!(f.send(command).await.0, 400);
    }
    for (path, value) in [
        ("credentialId", "other"),
        ("revision", "other"),
        ("assets[0].state", "processing"),
    ] {
        let original: String =
            sqlx::query_scalar("SELECT payload FROM request_audits WHERE id = ?")
                .bind(&id)
                .fetch_one(&local.pool)
                .await
                .unwrap();
        sqlx::query("UPDATE request_audits SET payload = json_set(payload, ?, ?) WHERE id = ?")
            .bind(format!("$.routeTrace.lumaMcpCreation.{path}"))
            .bind(value)
            .bind(&id)
            .execute(&local.pool)
            .await
            .unwrap();
        let mut command = f.command("prepare", None);
        command["request"] = input("image", &id, false);
        assert_eq!(
            f.send(command).await.0,
            if path == "revision" { 409 } else { 400 }
        );
        sqlx::query("UPDATE request_audits SET payload = ? WHERE id = ?")
            .bind(original)
            .bind(&id)
            .execute(&local.pool)
            .await
            .unwrap();
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM request_audits")
        .fetch_one(&local.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    assert_eq!(f.peer.calls.lock().unwrap().len(), before);
    f.close().await;
}

#[tokio::test]
async fn luma_mcp_creation_reuse_wrong_open_workspace_is_saved_but_never_generates() {
    let mut values = source_replies("image");
    let mut wrong = opened();
    wrong["workspace_id"] = json!("other");
    values.push(wrong);
    let f = Fixture::new(values).await;
    let root = source(&f, "image").await;
    let request = input("image", &root, false);
    let id = prepare(&f, &request).await;
    let task = start(&f, &id, &request).await;
    assert_eq!(task["state"], "unconfirmed");
    assert_eq!(task["sessionId"], "s1");
    assert!(task["runId"].is_null());
    assert_eq!(task["error"], "luma_mcp_invalid_response");
    let calls = f.peer.calls.lock().unwrap().clone();
    assert_eq!(
        calls
            .iter()
            .filter(|(_, v)| v["params"]["name"] == "luma_generate_image")
            .count(),
        1
    );
    f.close().await;
}

#[test]
fn luma_mcp_creation_reuse_rejects_ambiguous_inputs_and_preserves_old_serialization() {
    let root = format!("luma-mcp-{}", uuid::Uuid::new_v4());
    for bad in [
        {
            let mut r = input("image", &root, false);
            r["options"]["sourceUrl"] = json!("https://cdn.example.com/a");
            r
        },
        {
            let mut r = input("video", &root, false);
            r["options"]["sourceMediaType"] = json!("video/mp4");
            r
        },
        {
            let mut r = input("image", &root, false);
            r["reuse"]["referenceAssetIds"] = json!(["a1"]);
            r
        },
        {
            let mut r = input("image", &root, true);
            r["reuse"]["referenceAssetIds"] = json!(["media:foreign"]);
            r
        },
        {
            let mut r = input("image", &root, false);
            r["reuse"]["sourceAssetId"] = json!("x".repeat(65));
            r
        },
        {
            let mut r = input("image", &root, true);
            r["options"]["referenceUrls"] = json!(vec!["https://cdn.example.com/a"; 16]);
            r
        },
    ] {
        let request: creation::Request = serde_json::from_value(bad).unwrap();
        assert!(request.validate().is_err());
    }
    let mut old: creation::Request = serde_json::from_value(
        json!({"kind":"image","prompt":"old","budgetCredits":20,"options":{}}),
    )
    .unwrap();
    let serialized = serde_json::to_value(&old).unwrap();
    assert!(serialized.get("reuse").is_none());
    old.reuse = None;
    assert_eq!(serde_json::to_value(&old).unwrap(), serialized);
}
