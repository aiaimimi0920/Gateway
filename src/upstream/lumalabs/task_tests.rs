//! Successful local and remote metadata must survive service serialization and bind context.
use super::{task_result::TaskResult, tests::assert_error};
use crate::upstream::lumalabs_runtime_helpers::prepare_lumalabs_browser_executor_service_input;
use serde_json::{json, Value};

fn result() -> Value {
    json!({"signedUrl":"https://asset.fixture.invalid/image.png","outputId":"output-1",
        "actionId":null,"resolvedActionType":"create_image_uni_1","realmId":"realm-fixture",
        "mediaOperation":"image","artifactField":"image","state":"completed"})
}
fn input() -> Value {
    json!({"baseUrl":"https://app.lumalabs.ai","realmId":"realm-fixture",
        "mediaOperation":"image","artifactField":"image","sessionToken":"synthetic-private",
        "actionBody":{"type":"create_image_uni_1"},"autoDiscoverActionType":false})
}
#[test]
fn luma_web_task_result_roundtrip_preserves_all_context_and_optional_action_id() {
    let expected = prepare_lumalabs_browser_executor_service_input(&input()).unwrap();
    for action_id in [Value::Null, json!("action-1")] {
        let mut value = result();
        value["actionId"] = action_id;
        let local = TaskResult::from_stdout(&value.to_string()).unwrap();
        local.validate(&expected).unwrap();
        let forwarded = local.into_value();
        assert_eq!(forwarded, value);
        TaskResult::from_value(forwarded)
            .unwrap()
            .validate(&expected)
            .unwrap();
    }
}
#[test]
fn luma_web_task_result_rejects_legacy_nonterminal_empty_or_malformed_success() {
    for field in [
        "signedUrl",
        "outputId",
        "resolvedActionType",
        "realmId",
        "mediaOperation",
        "artifactField",
        "state",
    ] {
        for value in [None, Some(json!(" ")), Some(json!(4))] {
            let mut payload = result();
            match value {
                Some(value) => {
                    payload[field] = value;
                }
                None => {
                    payload.as_object_mut().unwrap().remove(field);
                }
            }
            assert_error(
                TaskResult::from_value(payload).unwrap_err(),
                "lumalabs_task_result_invalid",
            );
        }
    }
    for state in ["finalizing", "failed", "pending", "Completed"] {
        let mut payload = result();
        payload["state"] = json!(state);
        assert_error(
            TaskResult::from_value(payload).unwrap_err(),
            "lumalabs_task_result_invalid",
        );
    }
}
#[test]
fn luma_web_task_result_rejects_foreign_context_and_explicit_action_type() {
    let expected = prepare_lumalabs_browser_executor_service_input(&input()).unwrap();
    for (field, value) in [
        ("realmId", "foreign-private"),
        ("mediaOperation", "video"),
        ("artifactField", "foreign-private"),
        ("resolvedActionType", "foreign-private"),
    ] {
        let mut payload = result();
        payload[field] = json!(value);
        assert_error(
            TaskResult::from_value(payload)
                .unwrap()
                .validate(&expected)
                .unwrap_err(),
            "lumalabs_task_result_invalid",
        );
    }
}
#[test]
fn luma_web_task_result_discovery_records_actual_type_without_claiming_model_binding() {
    let mut prepared = prepare_lumalabs_browser_executor_service_input(&input()).unwrap();
    prepared.auto_discover_action_type = true;
    let mut payload = result();
    payload["resolvedActionType"] = json!("discovered-fixture");
    let task = TaskResult::from_value(payload).unwrap();
    task.validate(&prepared).unwrap();
    assert_eq!(
        task.into_value()["resolvedActionType"],
        "discovered-fixture"
    );
}
#[tokio::test]
async fn luma_web_task_result_local_worker_does_not_accept_legacy_url_success() {
    assert_error(
        super::tests::run("legacy").await.unwrap_err(),
        "lumalabs_task_result_invalid",
    );
}
