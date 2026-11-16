//! 三类决策的请求和回复合同；不联网，不将合成结果算作账号验收。
use super::{systemone_request as request, systemone_response as response};
use serde_json::{json, Value};

fn body() -> Value {
    json!({"model":"typesafe/jev","state":{"text":"合成内容"},"questions":{
        "yes":{"type":"noul","instructions":["是否相关"],"criteria":{"true":null,"false":{"hint":"无关"}}},
        "pick":{"type":"choice","criteria":{"a":null,"b":["第二种"]}},
        "rate":{"type":"score","criteria":[{"level":"低"},["高"]]}
    }})
}

fn reply() -> Value {
    json!({"model":"typesafe/jev-1.14.0","answers":{
        "yes":{"type":"noul","noul":0.9},
        "pick":{"type":"choice","choice":"b","confidence":0.7,"probabilities":{"a":0.2,"b":0.8}},
        "rate":{"type":"score","score":0.667,"confidence":0.5,
            "legend":{"0":{"level":"低"},"1":["高"]},"probabilities":{"0":0.333,"1":0.667}}
    },"usage":{"input_tokens":84,"output_tokens":3}})
}

#[test]
fn commandcode_systemone_structured_values_and_named_answers() {
    let body = body();
    let questions = request::validate(&body).unwrap();
    response::validate(&reply(), questions).unwrap();
    assert!(request::projection(&body).contains("合成内容"));
    let usage = response::usage(&reply()).unwrap();
    assert_eq!(
        (
            usage.prompt_tokens,
            usage.completion_tokens,
            usage.total_tokens
        ),
        (84, 3, 87)
    );
    for state in [json!(""), json!([]), json!({})] {
        let mut body = body.clone();
        body["state"] = state;
        assert!(request::validate(&body).is_ok());
    }
}

#[test]
fn commandcode_systemone_rejects_invalid_or_uncontracted_inputs() {
    for (pointer, value) in [
        ("/model", json!(" ")),
        ("/state", json!(null)),
        ("/state", json!(42)),
        ("/questions", json!({})),
        ("/questions/yes/type", json!("box")),
        ("/questions/yes/instructions", json!(true)),
        ("/questions/yes/criteria", json!({"maybe":"unknown"})),
        ("/questions/pick/criteria", json!({})),
        ("/questions/pick/criteria/a", json!(1)),
        ("/questions/rate/criteria", json!(["one"])),
        ("/questions/rate/criteria", json!(["low", null])),
    ] {
        let mut body = body();
        *body.pointer_mut(pointer).unwrap() = value;
        assert!(request::validate(&body).is_err(), "{pointer}");
    }
    for (field, value) in [
        ("stream", json!(true)),
        ("assets", json!({})),
        ("tools", json!([])),
    ] {
        let mut body = body();
        body[field] = value;
        assert!(request::validate(&body).is_err(), "{field}");
    }
    let mut body = body();
    body["questions"] = Value::Object(
        (0..100)
            .map(|i| (i.to_string(), json!({"type":"noul"})))
            .collect(),
    );
    assert!(request::validate(&body).is_ok());
    body["questions"]["overflow"] = json!({"type":"noul"});
    assert!(request::validate(&body).is_err());
}

#[test]
fn commandcode_systemone_rejects_bad_answers_and_usage_without_secret_echo() {
    let body = body();
    let questions = request::validate(&body).unwrap();
    for (pointer, value) in [
        ("/model", json!("")),
        ("/answers", json!({})),
        ("/answers/yes/type", json!("choice")),
        ("/answers/yes/noul", json!(1.01)),
        ("/answers/pick/choice", json!("unknown-secret")),
        ("/answers/pick/confidence", json!(-0.1)),
        ("/answers/pick/probabilities", json!({"a":0.2,"other":0.8})),
        ("/answers/rate/score", json!(2.0)),
        ("/answers/rate/legend/0", json!("different")),
        ("/usage/input_tokens", json!(-1)),
        ("/usage/output_tokens", json!(0.5)),
        ("/usage", json!({"input_tokens":u64::MAX,"output_tokens":1})),
    ] {
        let mut reply = reply();
        *reply.pointer_mut(pointer).unwrap() = value;
        let error = response::validate(&reply, questions).unwrap_err();
        assert_eq!(error.http_status, Some(502), "{pointer}");
        assert!(!error.retryable && !error.message.contains("unknown-secret"));
        assert!(matches!(
            error.fallback_hint,
            crate::error::FallbackHint::Abort { .. }
        ));
    }
}

fn payload() -> crate::routing::candidate::ProviderAccountPayload {
    let mut payload = serde_json::from_value(json!({
        "adapter":"openai_compatible", "auth_mode":super::AUTH_MODE,
        "base_url":super::BASE, "api_key":"synthetic-systemone"
    }))
    .unwrap();
    let catalog = serde_json::from_value(json!({
        "sourceUrl":super::SOURCE,"checkedAt":"2026-11-14T00:00:00Z",
        "models":["upstream"],"supportedEndpoints":{"upstream":["/systemone"]}
    }))
    .unwrap();
    super::catalog::configure(&mut payload, &catalog);
    payload
}

#[test]
fn commandcode_systemone_plan_preserves_json_and_refuses_policy_drift() {
    let mut body = body();
    body["stream"] = false.into();
    let original = super::systemone::normalize(body.clone()).unwrap();
    let payload = payload();
    let plan = super::runtime::plan(&payload, &original, "upstream", false).unwrap();
    assert_eq!(plan.method, rquest::Method::POST);
    assert_eq!(plan.url, format!("{}/systemone", super::BASE));
    assert!(plan.query.is_empty());
    assert_eq!(plan.response_kind, super::EndpointKind::SystemOne);
    let sent = plan.body.unwrap();
    assert_eq!(sent["model"], "upstream");
    assert_eq!(sent["state"], body["state"]);
    assert_eq!(sent["questions"], body["questions"]);
    assert!(sent.get("stream").is_none() && sent.get("messages").is_none());
    assert!(!crate::pipeline::request_budget::replay_safe(&original));
    assert!(super::runtime::plan(&payload, &original, "upstream", true).is_err());
    for mutation in 0..7 {
        let mut request = original.clone();
        match mutation {
            0 => request.messages[0].content = vec![],
            1 => request.messages.push(request.messages[0].clone()),
            2 => request.messages[0].role = crate::protocol::canonical::MessageRole::System,
            3 => {
                request.extra.insert("max_tokens".into(), 42.into());
            }
            4 => request.stream = true,
            5 => request.tool_choice = Some(json!("auto")),
            _ => request.previous_response_id = Some("opaque".into()),
        }
        let error = super::runtime::plan(&payload, &request, "upstream", false).unwrap_err();
        assert_eq!(
            error.code.as_deref(),
            Some("commandcode_systemone_policy_conflict"),
            "{mutation}"
        );
    }
}
