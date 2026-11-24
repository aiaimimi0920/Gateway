//! Non-review questions retain the user's JSON answer without guessing the supplier's answer schema.
use super::*;

fn question(kind: &str) -> Value {
    let mut value = pending("q1", &[]);
    value["pending"]["interaction"] = match kind {
        "confirm" => {
            json!({"kind":kind,"invocation_id":"q1","message":"Continue?","destructive":true})
        }
        "text_input" => {
            json!({"kind":kind,"invocation_id":"q1","prompt":"Your title?","placeholder":"Title"})
        }
        "select" => json!({"kind":kind,"invocation_id":"q1","message":"Choose","min":1,"max":1,
            "options":[{"asset_id":"a1","type":"image","state":"ready","url":"https://cdn.example.com/a.png"}]}),
        _ => json!({"kind":kind,"invocation_id":"q1","message":"Approve this request?"}),
    };
    value
}

#[tokio::test]
async fn luma_mcp_questions_send_explicit_answers_once_on_original_run() {
    for (kind, result) in [
        ("approve", json!(true)),
        ("confirm", json!({"confirmed":false})),
        ("text_input", json!("A title")),
        ("select", json!(["a1"])),
    ] {
        let f = Fixture::new(vec![
            opened(),
            started(),
            question(kind),
            usage(true),
            json!({"status":"delivered","run_id":"r1","session_id":"s1","seq":2}),
            page(3, false),
        ])
        .await;
        let view = task(&f).await;
        assert_eq!(view["review"]["invocationId"], "q1", "{kind}");
        let mut body = answer(&f, &view);
        body["answer"]["result"] = result.clone();
        let mut unconfirmed = body.clone();
        unconfirmed["chargeConfirmed"] = json!(false);
        assert_eq!(f.send(unconfirmed).await.0, 400);
        if kind == "select" {
            for result in [
                json!([]),
                json!(["other"]),
                json!(["a1", "artifact:a1"]),
                json!({"ids":["a1"]}),
            ] {
                let mut invalid = body.clone();
                invalid["answer"]["result"] = result;
                assert_eq!(f.send(invalid).await.0, 400);
            }
            assert!(tool_calls(&f, "luma_run_respond").is_empty());
        }
        let (status, sent) = f.send(body.clone()).await;
        assert_eq!(status, 200, "{kind}: {sent}");
        assert_eq!(sent["task"]["responses"][0]["status"], "delivered");
        assert_ne!(f.send(body).await.0, 200);
        let calls = tool_calls(&f, "luma_run_respond");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0]["run_id"], "r1");
        assert_eq!(calls[0]["result"], result);
        f.close().await;
    }
}

#[test]
fn luma_mcp_questions_correlate_session_waiting_without_inventing_a_question() {
    let direct = question("text_input")["pending"].clone();
    let mut page = page(3, false);
    page["waiting"] = json!([{"kind":"gate","run_id":"r1","invocation_id":"q1","answer_type":"user.tool_result"}]);
    let parsed = creation::select_review(&direct, Some(&page), "r1")
        .unwrap()
        .unwrap();
    assert!(parsed.answerable());
    assert_eq!(parsed.kind, "text_input");
    assert!(creation::select_review(&Value::Null, Some(&page), "r1")
        .unwrap()
        .is_none());
    page["waiting"][0]["invocation_id"] = json!("other");
    assert!(creation::select_review(&direct, Some(&page), "r1")
        .unwrap()
        .is_none());
    page["waiting"][0]["invocation_id"] = json!("q1");
    page["waiting"][0]["kind"] = json!("custom_tool");
    assert!(creation::select_review(&direct, Some(&page), "r1")
        .unwrap()
        .is_none());
}

#[test]
fn luma_mcp_questions_preserve_context_and_reject_invalid_media_or_missing_fields() {
    for kind in ["approve", "confirm", "text_input", "select"] {
        let mut value = question(kind)["pending"].clone();
        let parsed = creation::review(&value, "r1").unwrap().unwrap();
        assert!(parsed.answerable(), "{kind}");
        if kind == "confirm" {
            assert_eq!(parsed.content["destructive"], true);
        }
        if kind == "select" {
            assert_eq!(parsed.assets[0]["id"], "a1");
        }
        value["truncated"] = json!(true);
        assert!(!creation::review(&value, "r1")
            .unwrap()
            .unwrap()
            .answerable());
    }
    for (key, val) in [
        ("url", json!("https://127.0.0.1/private")),
        ("state", json!("processing")),
    ] {
        let mut value = question("select")["pending"].clone();
        value["interaction"]["options"][0][key] = val;
        assert!(creation::review(&value, "r1").is_err());
    }
    let mut value = question("confirm")["pending"].clone();
    value["interaction"]
        .as_object_mut()
        .unwrap()
        .remove("destructive");
    assert!(creation::review(&value, "r1").is_err());
    let mut value = question("select")["pending"].clone();
    value["interaction"]["max"] = json!(0);
    assert!(creation::review(&value, "r1").is_err());
}
