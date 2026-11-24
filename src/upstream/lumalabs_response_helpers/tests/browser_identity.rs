//! Account browser identity must survive both local stdin and the remote executor hop.
use super::{image_generation_request, make_payload, prepare_lumalabs_execution_context, Duration};
use crate::upstream::lumalabs_runtime_helpers::{
    build_lumalabs_browser_executor_payload_from_prepared,
    prepare_lumalabs_browser_executor_service_input,
    serialize_lumalabs_browser_worker_input_from_prepared,
};
use serde_json::{json, Value};

#[test]
fn luma_web_account_user_agent_reaches_local_and_remote_browser() {
    for name in ["user-agent", "User-Agent"] {
        let mut payload = make_payload("lumalabs_compatible", "https://app.lumalabs.ai");
        payload
            .headers
            .insert(name.into(), "FixtureBrowser/1.0".into());
        let request = image_generation_request(json!({"prompt":"identity fixture"}));
        let plan = prepare_lumalabs_execution_context(
            &payload,
            &request,
            "uni-1",
            Duration::from_secs(30),
        )
        .unwrap();
        let local: Value = serde_json::from_slice(
            &serialize_lumalabs_browser_worker_input_from_prepared(&plan.browser_input, None)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(local["userAgent"], "FixtureBrowser/1.0");
        let remote =
            build_lumalabs_browser_executor_payload_from_prepared(&plan.browser_input, None);
        assert_eq!(remote["userAgent"], "FixtureBrowser/1.0");
        let received = prepare_lumalabs_browser_executor_service_input(&remote).unwrap();
        let forwarded: Value = serde_json::from_slice(
            &serialize_lumalabs_browser_worker_input_from_prepared(&received, None).unwrap(),
        )
        .unwrap();
        assert_eq!(forwarded["userAgent"], "FixtureBrowser/1.0");
    }
}

#[test]
fn luma_web_legacy_account_keeps_browser_default_identity() {
    let payload = make_payload("lumalabs_compatible", "https://app.lumalabs.ai");
    let request = image_generation_request(json!({"prompt":"identity fixture"}));
    let plan =
        prepare_lumalabs_execution_context(&payload, &request, "uni-1", Duration::from_secs(30))
            .unwrap();
    let local: Value = serde_json::from_slice(
        &serialize_lumalabs_browser_worker_input_from_prepared(&plan.browser_input, None).unwrap(),
    )
    .unwrap();
    assert!(local.get("userAgent").is_none());
    let remote = build_lumalabs_browser_executor_payload_from_prepared(&plan.browser_input, None);
    assert!(remote.get("userAgent").is_none());
    let received = prepare_lumalabs_browser_executor_service_input(&remote).unwrap();
    let forwarded: Value = serde_json::from_slice(
        &serialize_lumalabs_browser_worker_input_from_prepared(&received, None).unwrap(),
    )
    .unwrap();
    assert!(forwarded.get("userAgent").is_none());
}
