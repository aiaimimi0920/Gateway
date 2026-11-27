//! Deterministic admission tests: no process, socket, database or upstream.
use super::*;
use crate::protocol::cursor_remote::{incremental, tool_schema::ToolKind};
use serde_json::json;
use tokio::{
    sync::{mpsc, oneshot, watch},
    time::Duration,
};

fn paused() -> (
    Tools,
    Publication,
    mpsc::Receiver<super::super::Round>,
    oneshot::Receiver<()>,
) {
    let tools = Tools::default();
    let (cancel, cancelled) = oneshot::channel();
    let (commands, receiver) = mpsc::channel(1);
    let epoch = uuid::Uuid::new_v4();
    *tools.waiting.lock() = Some(Session {
        id: uuid::Uuid::new_v4(),
        epoch,
        published: false,
        binding: "key/account/revision/ws".into(),
        signature: "original-policy-and-images".into(),
        chat: false,
        family: crate::protocol::canonical::ProtocolFamily::OpenAi,
        tools: vec![],
        prefix: vec![
            json!({"role":"user","content":"hello"}),
            json!({"type":"function_call","call_id":"call_one","name":"echo","arguments":"{}"}),
        ],
        pending: "call_one".into(),
        pending_kind: ToolKind::Function,
        expires: Instant::now() + Duration::from_secs(60),
        cancel: Arc::new(super::super::Cancel(parking_lot::Mutex::new(Some(cancel)))),
        settlement: Arc::new(parking_lot::Mutex::new(watch::channel(false).0)),
        events: mpsc::channel(1).1,
        commands,
    });
    let publication = Publication {
        epoch: Some(epoch),
        waiting: tools.waiting.clone(),
    };
    (tools, publication, receiver, cancelled)
}

fn body() -> serde_json::Value {
    json!({"model":"cursor-personal","store":false,"stream":true,
        "previous_response_id":"resp_one",
        "input":[{"type":"function_call_output","call_id":"call_one","output":"ok"}]})
}
fn parse(body: serde_json::Value, cap: &Continuation) -> Result<IncrementalRequest, GatewayError> {
    incremental::parse(&crate::protocol::responses::normalize_responses(body)?, cap)
}
fn capability(publication: &Publication) -> Continuation {
    assert!(publication.publish_on_connection());
    publication.continuation("resp_one").unwrap()
}

#[test]
fn cursor_remote_incremental_capability_requires_publication_and_does_not_own_pause() {
    let (tools, publication, _commands, mut cancelled) = paused();
    assert!(publication.continuation("resp_one").is_none());
    let cap = capability(&publication);
    drop(cap.clone());
    assert!(cancelled.try_recv().is_err());
    assert!(tools.waiting.lock().is_some());
    drop(publication);
    assert!(cancelled.try_recv().is_ok());
    assert!(tools.waiting.lock().is_none());
    assert!(tools
        .resume(
            "key/account/revision/ws",
            &parse(body(), &cap).unwrap().into()
        )
        .is_err());
}

#[test]
fn cursor_remote_incremental_raw_contract_never_silently_changes_policy() {
    let (_tools, publication, _commands, _cancelled) = paused();
    let cap = capability(&publication);
    for (field, value) in [
        ("tools", json!([])),
        ("instructions", json!("")),
        ("tool_choice", json!("auto")),
        ("parallel_tool_calls", json!(false)),
        ("reasoning", json!({"effort":"none"})),
        ("store", json!(true)),
        ("stream", json!(false)),
        ("stream_id", json!("main")),
        ("background", json!(false)),
        ("generate", json!(false)),
        ("max_output_tokens", json!(5)),
        ("previous_response_id", json!("resp_other")),
        ("previous_response_id", json!(null)),
    ] {
        let mut request = body();
        request[field] = value;
        assert!(parse(request, &cap).is_err(), "{field}");
    }
    for input in [
        json!([]),
        json!("ok"),
        json!([{"role":"user","content":"new task"}]),
        json!([{"type":"function_call_output","id":"call_one","output":"ok"}]),
        json!([{"type":"function_call_output","call_id":"","output":"ok"}]),
        json!([{"type":"function_call_output","call_id":"call_one","output":"ok","extra":true}]),
        json!([{"type":"function_call_output","call_id":"call_one","output":"x".repeat(65537)}]),
        json!([{"type":"function_call_output","call_id":"call_one","output":"\0".repeat(30000)}]),
    ] {
        let mut request = body();
        request["input"] = input;
        assert!(parse(request, &cap).is_err());
    }
    let mut request = body();
    request["input"][0]["output"] = json!([
        {"type":"input_text","text":" \r\n中文"}, {"type":"input_text","text":"&lt;\n"}
    ]);
    assert_eq!(parse(request, &cap).unwrap().output.text, " \r\n中文&lt;\n");
    // Without the internal socket capability, the identical raw body is rejected.
    let req = crate::protocol::responses::normalize_responses(body()).unwrap();
    assert!(crate::protocol::cursor_remote::ingress::parse(&req).is_err());
    assert!(parse(body(), &cap).is_ok());
}

#[test]
fn cursor_remote_incremental_mismatch_expiry_and_limits_do_not_consume_pause() {
    let (tools, publication, _commands, _cancelled) = paused();
    let cap = capability(&publication);
    let valid: ToolTurn = parse(body(), &cap).unwrap().into();
    tools.waiting.lock().as_mut().unwrap().published = false;
    assert!(tools.resume("key/account/revision/ws", &valid).is_err());
    tools.waiting.lock().as_mut().unwrap().published = true;
    for binding in [
        "other/account/revision/ws",
        "key/other/revision/ws",
        "key/account/new/ws",
        "key/account/revision/other-ws",
    ] {
        assert!(tools.resume(binding, &valid).is_err());
    }
    for (field, value) in [
        ("call_id", json!("other")),
        ("type", json!("custom_tool_call_output")),
    ] {
        let mut request = body();
        request["input"][0][field] = value;
        assert!(tools
            .resume(
                "key/account/revision/ws",
                &parse(request, &cap).unwrap().into()
            )
            .is_err());
    }
    let (_other, other_publication, _other_commands, _other_cancelled) = paused();
    let mut foreign = parse(body(), &capability(&other_publication)).unwrap();
    // Even matching epochs cannot authorize a capability from another registry.
    foreign.continuation.epoch = cap.epoch;
    assert!(tools
        .resume("key/account/revision/ws", &foreign.into())
        .is_err());
    {
        let mut waiting = tools.waiting.lock();
        waiting.as_mut().unwrap().epoch = uuid::Uuid::new_v4();
    }
    assert!(tools.resume("key/account/revision/ws", &valid).is_err());
    {
        let mut waiting = tools.waiting.lock();
        let session = waiting.as_mut().unwrap();
        session.epoch = cap.epoch;
        session.expires = Instant::now() - Duration::from_secs(1);
    }
    assert!(publication.continuation("resp_one").is_none());
    assert!(tools.resume("key/account/revision/ws", &valid).is_err());
    let original = {
        let mut waiting = tools.waiting.lock();
        let session = waiting.as_mut().unwrap();
        session.expires = Instant::now() + Duration::from_secs(60);
        std::mem::replace(
            &mut session.prefix,
            vec![json!({"content":"x".repeat(384 * 1024 - 32)})],
        )
    };
    assert!(tools.resume("key/account/revision/ws", &valid).is_err());
    tools.waiting.lock().as_mut().unwrap().prefix = vec![json!({}); 256];
    assert!(tools.resume("key/account/revision/ws", &valid).is_err());
    tools.waiting.lock().as_mut().unwrap().prefix = original.clone();
    let expires = tools.waiting.lock().as_ref().unwrap().expires;
    let mut session = tools.resume("key/account/revision/ws", &valid).unwrap();
    assert!(tools.waiting.lock().is_none());
    assert_eq!(session.signature, "original-policy-and-images");
    assert_eq!(session.expires, expires); // Delta admission cannot refresh the Run TTL.
    assert_eq!(session.prefix.pop().unwrap(), body()["input"][0]);
    assert_eq!(session.prefix, original);
    assert!(tools.resume("key/account/revision/ws", &valid).is_err());
}

#[test]
fn cursor_remote_incremental_closed_command_channel_cannot_be_claimed() {
    let (tools, publication, commands, _cancelled) = paused();
    let request: ToolTurn = parse(body(), &capability(&publication)).unwrap().into();
    drop(commands);
    assert!(publication.continuation("resp_one").is_none());
    assert!(tools.resume("key/account/revision/ws", &request).is_err());
    assert!(tools.waiting.lock().is_some());
    drop(publication);
    assert!(tools.waiting.lock().is_none());
}
