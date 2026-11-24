//! Explicit review responses cross management auth, SQLite CAS and the synthetic MCP peer.
use super::*;
#[path = "creation_review_contract_tests.rs"]
mod contract_tests;
#[path = "creation_question_tests.rs"]
mod question_tests;

fn pending(invocation: &str, ids: &[&str]) -> Value {
    let mut value = progress("needs_input", false, json!([]));
    value["pending"] = json!({"type":"agent.gate","run_id":"run:r1","interaction":{
        "kind":"review","invocation_id":invocation,"tool":"review","params":{"question":"Review this draft"},
        "answer_format":{"verdict_schema":{"type":"object","properties":{"decision":{"enum":["accept","revise"]}},"required":["decision"],"additionalProperties":false},
            "revision_targets":["headline"]},
        "text":[{"target":"headline","label":"Title","text":"A blue circle"}],
        "show":ids.iter().map(|id| json!({"target":"headline","asset_id":id})).collect::<Vec<_>>()}});
    value
}
fn assets(ids: &[&str]) -> Value {
    json!({"assets":ids.iter().map(|id| json!({"asset_id":id,"type":if *id=="v1" {"video"} else {"image"},
        "has_image":true,"state":"ready","url":format!("https://cdn.example.com/{id}")})).collect::<Vec<_>>()})
}
fn page(seq: u64, more: bool) -> Value {
    json!({"session_id":"s1","status":"requires_action","timed_out":false,"next_after_seq":seq,
        "has_more":more,"events":[{"type":"message","text":"Next review"}]})
}
async fn task(f: &Fixture) -> Value {
    let task = f.act("prepare", None).await;
    let id = task["id"].as_str().unwrap();
    f.act("start", Some(id)).await;
    f.act("read", Some(id)).await
}
fn answer(f: &Fixture, task: &Value) -> Value {
    let mut body = f.command("respond", task["id"].as_str());
    body["chargeConfirmed"] = json!(true);
    body["answer"] = json!({"invocationId":task["review"]["invocationId"],
        "fingerprint":task["review"]["fingerprint"],"result":{"decision":"accept"}});
    body
}
fn tool_calls(f: &Fixture, name: &str) -> Vec<Value> {
    f.peer
        .calls
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, v)| v["params"]["name"] == name)
        .map(|(_, v)| v["params"]["arguments"].clone())
        .collect()
}

#[tokio::test]
async fn luma_mcp_creation_review_displays_assets_sends_once_and_follows_paged_session() {
    let f = Fixture::new(vec![opened(), started(), pending("g1", &["a1","a2","v1","a4","a5"]),
        assets(&["a1","a2","v1","a4"]), assets(&["a5"]), usage(true),
        json!({"status":"delivered","run_id":"r1","session_id":"s1","seq":7}),
        json!({"session_id":"s1","status":"working","timed_out":true,"next_after_seq":7,"events":[]}),
        page(8,true), pending("g2", &[]), usage(true), page(9,false), pending("g2", &[]), usage(true),
        json!({"status":"delivered","run_id":"r1","session_id":"s1","seq":10}), page(11,false)]).await;
    let review = task(&f).await;
    let id = review["id"].as_str().unwrap();
    assert_eq!(review["review"]["assets"].as_array().unwrap().len(), 5);
    assert!(review["assets"].as_array().unwrap().is_empty());
    let (status, sent) = f.send(answer(&f, &review)).await;
    assert_eq!(status, 200, "{sent}");
    assert_eq!(sent["task"]["responses"][0]["status"], "delivered");
    assert_eq!(sent["task"]["afterSeq"], 7);
    assert!(sent["task"]["review"].is_null());
    let next = f.act("read", Some(id)).await;
    let before = f.peer.calls.lock().unwrap().len();
    assert_eq!(f.send(answer(&f, &next)).await.0, 400);
    assert_eq!(f.peer.calls.lock().unwrap().len(), before);
    let next = f.act("read", Some(id)).await;
    assert_eq!(f.send(answer(&f, &next)).await.0, 200);
    let responses = tool_calls(&f, "luma_run_respond");
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0]["run_id"], "r1");
    assert_eq!(responses[0]["invocation_id"], "g1");
    assert_eq!(responses[0]["result"], json!({"decision":"accept"}));
    assert_ne!(
        responses[0]["idempotency_key"],
        responses[1]["idempotency_key"]
    );
    let cursors: Vec<_> = tool_calls(&f, "luma_session_await")
        .iter()
        .map(|v| v["after_seq"].as_u64().unwrap())
        .collect();
    assert_eq!(cursors, vec![7, 7, 8, 10]);
    assert_eq!(tool_calls(&f, "luma_generate_image").len(), 1);
    f.close().await;
}

#[tokio::test]
async fn luma_mcp_creation_review_requires_exact_question_confirmation_and_ready_media() {
    let f = Fixture::new(vec![opened(), started(), pending("g1", &[]), usage(true)]).await;
    let review = task(&f).await;
    let before = f.peer.calls.lock().unwrap().len();
    for (path, value) in [
        ("/answer/invocationId", json!("other")),
        ("/answer/fingerprint", json!("old")),
        ("/chargeConfirmed", json!(false)),
        ("/answer/result", json!("x".repeat(16385))),
    ] {
        let mut request = answer(&f, &review);
        *request.pointer_mut(path).unwrap() = value;
        assert_eq!(f.send(request).await.0, 400);
    }
    let mut wrong_action = answer(&f, &review);
    wrong_action["action"] = json!("read");
    assert_eq!(f.send(wrong_action).await.0, 400);
    assert_eq!(f.peer.calls.lock().unwrap().len(), before);
    f.close().await;
    let mut unavailable = assets(&["a1"]);
    unavailable["assets"][0]["url"] = Value::Null;
    let f = Fixture::new(vec![
        opened(),
        started(),
        pending("g1", &["a1"]),
        unavailable,
        usage(true),
    ])
    .await;
    let review = task(&f).await;
    assert_eq!(f.send(answer(&f, &review)).await.0, 400);
    assert!(tool_calls(&f, "luma_run_respond").is_empty());
    f.close().await;
}

#[tokio::test]
async fn luma_mcp_creation_review_uncertain_delivery_survives_restart_without_replay() {
    let mut f = Fixture::new(vec![
        opened(),
        started(),
        pending("g1", &[]),
        usage(true),
        json!({"status":"delivered","session_id":"foreign","run_id":"r1","seq":2}),
        pending("g1", &[]),
        usage(true),
    ])
    .await;
    let review = task(&f).await;
    let sent = f.send(answer(&f, &review)).await.1["task"].clone();
    assert_eq!(sent["state"], "unconfirmed");
    assert_eq!(sent["responses"][0]["status"], "unconfirmed");
    browser_fixture::close(&f.state).await;
    f.state = browser_fixture::state(&f.root).await;
    f.grant = support::grant_console_secret_access(&f.state).await;
    let read = f.act("read", review["id"].as_str()).await;
    assert_eq!(read["state"], "needs_input");
    assert_eq!(f.send(answer(&f, &read)).await.0, 409);
    assert_eq!(tool_calls(&f, "luma_run_respond").len(), 1);
    assert!(tool_calls(&f, "luma_session_await").is_empty());
    f.close().await;
}

#[tokio::test]
async fn luma_mcp_creation_review_delivery_is_saved_before_follow_failure() {
    let f = Fixture::new(vec![opened(),started(),pending("g1",&[]),usage(true),
        json!({"status":"delivered","session_id":"s1","run_id":"r1","seq":5}),
        json!({"session_id":"other","status":"working","timed_out":true,"next_after_seq":5,"events":[]}),
        page(6,false),progress("completed",false,json!([])),usage(false)]).await;
    let review = task(&f).await;
    let sent = f.send(answer(&f, &review)).await.1["task"].clone();
    assert_eq!(sent["responses"][0]["status"], "delivered");
    assert_eq!(sent["afterSeq"], 5);
    assert_eq!(sent["error"], "luma_mcp_invalid_response");
    let read = f.act("read", review["id"].as_str()).await;
    assert_eq!(read["state"], "completed");
    assert_eq!(read["afterSeq"], 6);
    assert_eq!(tool_calls(&f, "luma_run_respond").len(), 1);
    let calls = tool_calls(&f, "luma_session_await");
    assert!(calls.iter().all(|v| v["after_seq"] == 5));
    f.close().await;
}

#[tokio::test]
async fn luma_mcp_creation_review_rejects_custom_tools_and_accepts_legacy_journals() {
    let mut custom = pending("g1", &[]);
    custom["pending"] = json!({"type":"agent.custom_tool_use","invocation_id":"g1","tool":"shell","params":{"secret":"do-not-expose"}});
    let f = Fixture::new(vec![opened(), started(), custom, usage(true)]).await;
    let prepared = f.act("prepare", None).await;
    let id = prepared["id"].as_str().unwrap();
    sqlx::query("UPDATE request_audits SET payload = json_remove(payload, '$.routeTrace.lumaMcpCreation.review', '$.routeTrace.lumaMcpCreation.responses', '$.routeTrace.lumaMcpCreation.afterSeq', '$.routeTrace.lumaMcpCreation.sessionUpdate') WHERE id = ?")
        .bind(id).execute(&f.state.local_runtime.as_ref().unwrap().pool).await.unwrap();
    f.act("start", Some(id)).await;
    let task = f.act("read", Some(id)).await;
    assert_eq!(task["review"]["kind"], "unsupported");
    assert!(!task.to_string().contains("do-not-expose"));
    let mut body = answer(&f, &task);
    body["answer"]["invocationId"] = json!("g1");
    assert_eq!(f.send(body).await.0, 400);
    assert!(tool_calls(&f, "luma_run_respond").is_empty());
    f.close().await;
}

#[tokio::test]
async fn luma_mcp_creation_review_disconnect_keeps_delivery_and_expired_intent_stays_consumed() {
    let mut responses = replies(vec![
        opened(),
        started(),
        pending("g1", &[]),
        usage(true),
        json!({"status":"delivered","session_id":"s1","run_id":"r1","seq":5}),
        page(7, false),
        pending("g1", &[]),
        usage(true),
    ]);
    responses[14].delay = std::time::Duration::from_millis(350);
    let mut f = Fixture::with_replies(responses).await;
    let review = task(&f).await;
    let body = answer(&f, &review);
    let (state, grant, endpoint) = (f.state.clone(), f.grant.clone(), f.peer.endpoint.clone());
    let waiter = tokio::spawn(async move {
        test_support::scope(
            endpoint,
            support::send_console_json(
                &state,
                Method::POST,
                "/v1/internal/gateway/console/luma-mcp-creation",
                body,
                Some(&grant),
                None,
            ),
        )
        .await
    });
    f.peer.wait_calls(15).await;
    waiter.abort();
    let _ = waiter.await;
    browser_fixture::close(&f.state).await;
    f.state = browser_fixture::state(&f.root).await;
    f.grant = support::grant_console_secret_access(&f.state).await;
    let id = review["id"].as_str().unwrap();
    let raw: String = sqlx::query_scalar("SELECT payload FROM request_audits WHERE id = ?")
        .bind(id)
        .fetch_one(&f.state.local_runtime.as_ref().unwrap().pool)
        .await
        .unwrap();
    let saved: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(
        saved["routeTrace"]["lumaMcpCreation"]["responses"][0]["status"],
        "delivered"
    );
    assert!(!raw.contains("accept"));
    assert!(tool_calls(&f, "luma_session_await").is_empty());
    // An OS interruption can leave a durable sending intent without its final receipt.
    sqlx::query("UPDATE request_audits SET payload = json_set(payload, '$.routeTrace.lumaMcpCreation.state', 'responding', '$.routeTrace.lumaMcpCreation.leaseUntil', 0, '$.routeTrace.lumaMcpCreation.responses[0].status', 'submitting') WHERE id = ?")
        .bind(id).execute(&f.state.local_runtime.as_ref().unwrap().pool).await.unwrap();
    let read = f.act("read", Some(id)).await;
    assert_eq!(read["responses"][0]["status"], "unconfirmed");
    assert_eq!(f.send(answer(&f, &read)).await.0, 409);
    assert_eq!(tool_calls(&f, "luma_run_respond").len(), 1);
    f.close().await;
}

#[tokio::test]
async fn luma_mcp_creation_review_ended_session_still_reads_pages_and_final_run() {
    let mut last_page = page(6, true);
    last_page["status"] = json!("ended");
    let mut final_page = page(7, false);
    final_page["status"] = json!("ended");
    let f = Fixture::new(vec![
        opened(),
        started(),
        pending("g1", &[]),
        usage(true),
        json!({"status":"delivered","session_id":"s1","run_id":"r1","seq":5}),
        last_page,
        final_page,
        progress(
            "completed",
            false,
            json!([{"asset_id":"a1","type":"image"}]),
        ),
        assets(&["a1"]),
        usage(false),
    ])
    .await;
    let review = task(&f).await;
    let sent = f.send(answer(&f, &review)).await.1["task"].clone();
    assert_eq!(sent["sessionUpdate"]["status"], "ended");
    assert_eq!(sent["sessionUpdate"]["has_more"], true);
    let read = f.act("read", review["id"].as_str()).await;
    assert_eq!(read["afterSeq"], 7);
    assert_eq!(read["state"], "completed");
    assert_eq!(read["assets"][0]["id"], "a1");
    assert_eq!(read["usage"]["settled"], true);
    assert_eq!(tool_calls(&f, "luma_run_respond").len(), 1);
    f.close().await;
}

#[tokio::test]
async fn luma_mcp_creation_review_answers_session_only_review_on_original_run() {
    let interaction = pending("g2", &["a1"])["pending"]["interaction"].clone();
    let waiting = json!({"kind":"gate","run_id":"run:r1","answer_type":"user.gate_verdict",
        "invocation_id":"g2","answer_format":interaction["answer_format"],
        "show":interaction["show"],"text":interaction["text"]});
    let mut foreign = waiting.clone();
    foreign["run_id"] = json!("run:foreign");
    foreign["invocation_id"] = json!("foreign-gate");
    let mut next = page(8, true);
    next["waiting"] = json!([waiting.clone(), foreign.clone()]);
    let mut final_page = page(9, false);
    final_page["waiting"] = json!([foreign, waiting]);
    let f = Fixture::new(vec![
        opened(), started(), pending("g1", &[]), usage(true),
        json!({"status":"delivered","run_id":"r1","session_id":"s1","seq":7}),
        json!({"session_id":"s1","status":"working","timed_out":true,"next_after_seq":7,"events":[]}),
        next, progress("running", true, json!([])), usage(true),
        final_page, progress("running", true, json!([])), assets(&["a1"]), usage(true),
        json!({"status":"delivered","run_id":"r1","session_id":"s1","seq":10}), page(11,false),
    ]).await;
    let first = task(&f).await;
    let id = first["id"].as_str();
    let (status, first_answer) = f.send(answer(&f, &first)).await;
    assert_eq!(status, 200);
    assert_eq!(first_answer["task"]["afterSeq"], 7);
    let partial = f.act("read", id).await;
    assert_eq!(partial["afterSeq"], 8);
    let mut premature = answer(&f, &partial);
    // Use a well-formed input so the test reaches management admission, not JSON extraction.
    premature["answer"]["invocationId"] = json!("g2");
    premature["answer"]["fingerprint"] = json!("0".repeat(64));
    assert_eq!(f.send(premature).await.0, 400);
    let second = f.act("read", id).await;
    assert_eq!(second["state"], "needs_input");
    assert_eq!(second["review"]["invocationId"], "g2");
    assert_eq!(second["review"]["assets"][0]["id"], "a1");
    let second_answer = answer(&f, &second);
    assert_eq!(f.send(second_answer.clone()).await.0, 200);
    assert_eq!(f.send(second_answer).await.0, 400);
    let responses = tool_calls(&f, "luma_run_respond");
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[1]["invocation_id"], "g2");
    assert_eq!(responses[1]["run_id"], "r1");
    assert_eq!(tool_calls(&f, "luma_generate_image").len(), 1);
    f.close().await;
}
