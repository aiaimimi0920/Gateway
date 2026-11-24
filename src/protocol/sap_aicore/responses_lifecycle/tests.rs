use super::*;
use crate::protocol::responses::normalize_responses;
use serde_json::json;

const LOCAL: &str = "resp_gw_ec7d911924c14ced8dc62a533f99ad9d";
const UPSTREAM: &str = "resp_sap_123";

pub(super) fn payload() -> ProviderAccountPayload {
    let mut value = super::super::tests::payload();
    value
        .extra_body
        .as_mut()
        .unwrap()
        .insert(config::NATIVE_RESPONSES.into(), Value::Bool(true));
    value
        .extra_body
        .as_mut()
        .unwrap()
        .insert(config::STORED_RESPONSES.into(), true.into());
    value
}

#[test]
fn sap_aicore_responses_lifecycle_paths_and_path_segment_admission() {
    let payload = payload();
    let id = UpstreamId::parse(UPSTREAM).unwrap();
    for (operation, method, tail) in [
        (Operation::Retrieve, Method::GET, ""),
        (Operation::Cancel, Method::POST, "/cancel"),
        (Operation::Delete, Method::DELETE, ""),
    ] {
        let plan = operation_plan(&payload, &id, operation).unwrap();
        assert_eq!(plan.method, method);
        assert_eq!(
            plan.url,
            format!("{}/responses/{UPSTREAM}{tail}", payload.base_url)
        );
        assert!(plan.query.is_empty() && plan.body.is_none());
    }
    let plan = operation_plan(&payload, &id, Operation::RetrieveStream(Some(7))).unwrap();
    assert_eq!(plan.method, Method::GET);
    assert_eq!(
        plan.query,
        vec![
            ("stream".into(), "true".into()),
            ("starting_after".into(), "7".into())
        ]
    );
    assert!(plan.body.is_none());
    assert!(operation_plan(&payload, &id, Operation::RetrieveStream(Some(-1))).is_err());
    for raw in [
        "", ".", "../id", "x%2Fy", "x\\y", "a?b", "a#b", " a", "中", LOCAL,
    ] {
        assert!(UpstreamId::parse(raw).is_none(), "{raw}");
    }
    assert!(UpstreamId::parse(&"a".repeat(257)).is_none());
    assert!(operation_plan(&super::super::tests::payload(), &id, Operation::Retrieve).is_err());
    let mut native_only = payload;
    native_only
        .extra_body
        .as_mut()
        .unwrap()
        .remove(config::STORED_RESPONSES);
    assert!(native_responses::selected(
        &native_only,
        crate::protocol::canonical::EndpointKind::Responses
    ));
    assert!(operation_plan(&native_only, &id, Operation::Retrieve).is_err());
}

#[test]
fn sap_aicore_responses_lifecycle_stored_create_rewrites_only_resolved_predecessor() {
    let payload = payload();
    let previous = UpstreamId::parse(UPSTREAM).unwrap();
    let mut raw = json!({"model":"public-model","input":"follow up", "store":true,
        "background":true,"stream":false,"previous_response_id":LOCAL});
    let mut canonical = normalize_responses(raw.clone()).unwrap();
    let plan = create_plan(
        &payload,
        &canonical,
        "deployment1",
        Some((LOCAL, &previous)),
    )
    .unwrap();
    assert_eq!(plan.method, Method::POST);
    assert_eq!(plan.url, format!("{}/responses", payload.base_url));
    assert!(plan.query.is_empty());
    assert_eq!(
        plan.body.unwrap(),
        json!({"input":"follow up","store":true,
        "background":true,"stream":false,"previous_response_id":UPSTREAM})
    );
    assert!(create_plan(&payload, &canonical, "deployment1", None).is_err());
    assert!(create_plan(
        &payload,
        &canonical,
        "other-deployment",
        Some((LOCAL, &previous))
    )
    .is_err());
    assert!(native_responses::plan(&payload, &canonical, "deployment1", false).is_err());
    canonical.previous_response_id = None;
    assert!(create_plan(
        &payload,
        &canonical,
        "deployment1",
        Some((LOCAL, &previous))
    )
    .is_err());
    raw["previous_response_id"] = Value::Null;
    raw["background"] = false.into();
    let sync = normalize_responses(raw.clone()).unwrap();
    let plan = create_plan(&payload, &sync, "deployment1", None).unwrap();
    assert!(plan.body.unwrap().get("previous_response_id").is_none());
    for mutation in [
        json!({"store":false}),
        json!({"background":"yes"}),
        json!({"conversation":"external"}),
    ] {
        let mut invalid = raw.clone();
        invalid
            .as_object_mut()
            .unwrap()
            .extend(mutation.as_object().unwrap().clone());
        let req = normalize_responses(invalid).unwrap();
        assert!(create_plan(&payload, &req, "deployment1", None).is_err());
    }
}

#[test]
fn sap_aicore_responses_lifecycle_pending_is_not_a_terminal_usage_observation() {
    let mut body =
        json!({"id":UPSTREAM,"object":"response","status":"queued","store":true,"output":[]});
    let (id, state) = observe(&body, None).unwrap();
    assert!(matches!(state, State::Pending));
    assert!(observe(&body, Some(&UpstreamId::parse("other").unwrap())).is_err());
    body["usage"] = json!({"input_tokens":2,"output_tokens":3,"total_tokens":5});
    assert!(observe(&body, Some(&id)).is_err());
    body["status"] = "completed".into();
    let (_, state) = observe(&body, Some(&id)).unwrap();
    assert!(matches!(state, State::Completed(Some(ref usage)) if usage.total_tokens == 5));
    body["status"] = "cancelled".into();
    assert!(matches!(observe(&body, Some(&id)).unwrap().1,
        State::Failed(Some(ref usage)) if usage.total_tokens == 5));
    body["usage"]["total_tokens"] = 100.into();
    assert!(observe(&body, Some(&id)).is_err());
    assert!(validate_deleted(
        &json!({"id":UPSTREAM,"object":"response.deleted","deleted":true}),
        &id
    )
    .is_ok());
    assert!(validate_deleted(
        &json!({"id":"other","object":"response.deleted","deleted":true}),
        &id
    )
    .is_err());
}
