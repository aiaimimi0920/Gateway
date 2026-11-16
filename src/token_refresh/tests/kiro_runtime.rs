//! The opt-in runtime keeps its profile identity through ordinary durable refresh and cache repair.
use super::{kiro_fixture::*, *};
use serde_json::{json, Value};
const RUNTIME: &str = "https://runtime.eu-central-1.kiro.dev";
const PROFILE: &str = "arn:aws:codewhisperer:eu-central-1:123456789012:profile/synthetic";

async fn runtime_fixture() -> LocalFixture {
    let fixture = fixture().await;
    let snapshot = fixture.runtime.route_config().snapshot();
    let mut value = serde_json::to_value(snapshot.document()).unwrap();
    let provider = &mut value["providers"][0];
    provider["base_url"] = json!(RUNTIME);
    provider["auth_mode"] = json!("kiro_oauth_runtime");
    provider["responses_path"] = json!("/");
    provider["extra_body"] = json!({"profileArn":PROFILE});
    provider["credentials"][0]["kiro_oauth"]["runtime_target"] =
        json!({"base_url":RUNTIME,"profile_arn":PROFILE});
    fixture
        .runtime
        .commit_document(
            snapshot.revision().id(),
            serde_json::from_value(value).unwrap(),
            None,
        )
        .await
        .unwrap();
    fixture
}

#[tokio::test]
async fn kiro_runtime_refresh_preserves_profile_and_separate_directory_through_rebuild() {
    let mut fixture = runtime_fixture().await;
    let mock = Mock::new(vec![response(true)]).await;
    refresh(&fixture, &mock, &mut Pending::new()).await;
    assert_eq!(mock.count(), 1);
    let saved = fixture.saved();
    assert_eq!(saved.api_key.as_deref(), Some("new-access"));
    let context = saved.kiro_oauth.as_ref().unwrap();
    assert_eq!(context.token_endpoint(), ENDPOINT);
    assert_eq!(context.runtime_target().unwrap().profile_arn(), PROFILE);
    fixture.runtime = RouteConfigRuntime::new_local(&fixture.config).unwrap();
    fixture.runtime.recover_startup().await.unwrap();
    let snapshot = fixture.runtime.route_config().snapshot();
    let payload = &snapshot
        .oauth_refresh_target("oauth", "account")
        .unwrap()
        .1
        .payload;
    assert_eq!(payload.base_url, RUNTIME);
    assert_eq!(payload.extra_body.as_ref().unwrap()["profileArn"], PROFILE);
    let headers = crate::upstream::headers::build_upstream_headers(payload);
    assert_eq!(headers["authorization"], "Bearer new-access");
    assert!(!headers.contains_key("tokentype"));
    refresh(&fixture, &mock, &mut Pending::new()).await;
    assert_eq!(mock.count(), 1);
    mock.stop().await;
}

#[tokio::test]
async fn kiro_runtime_cache_repair_rejects_cross_profile_mode_and_destination() {
    let fixture = runtime_fixture().await;
    let request = request(&fixture).1;
    let result = request
        .result(
            "new-access",
            Some("new-refresh"),
            1,
            "2099-01-01T00:00:00Z",
            None,
        )
        .unwrap();
    let record = crate::console::refresh_grants::GrantRecord::sending(&request)
        .with_result(&result)
        .unwrap();
    let snapshot = fixture.runtime.route_config().snapshot();
    let payload = &snapshot
        .oauth_refresh_target("oauth", "account")
        .unwrap()
        .1
        .payload;
    let mut raw = cache();
    raw["api_base_url"] = json!(RUNTIME);
    raw["account_payload"]["kiro_oauth"] =
        serde_json::to_value(fixture.saved().kiro_oauth).unwrap();
    raw["account_payload"]["auth_mode"] = json!("kiro_oauth_runtime");
    raw["account_payload"]["extra_body"] = json!({"profileArn":PROFILE});
    let patch = |v: &Value| {
        super::super::server_cache::patch_from_grant(&v.to_string(), &record, payload).unwrap()
    };
    assert!(patch(&raw).is_some());
    for (path, value) in [
        ("/api_base_url", json!(BASE)),
        ("/account_payload/auth_mode", json!("kiro_api_key")),
        (
            "/account_payload/extra_body/profileArn",
            json!(PROFILE.replace("synthetic", "other")),
        ),
        (
            "/account_payload/kiro_oauth/runtime_target/profile_arn",
            json!(PROFILE.replace("synthetic", "other")),
        ),
    ] {
        let mut changed = raw.clone();
        *changed.pointer_mut(path).unwrap() = value;
        assert!(patch(&changed).is_none(), "accepted {path}");
    }
}

#[tokio::test]
async fn kiro_runtime_profile_change_retires_inflight_refresh_without_reusing_consumed_grant() {
    let fixture = runtime_fixture().await;
    let old = owner(&fixture);
    let mut reply = response(true);
    reply.header_delay = Duration::from_millis(150);
    let mock = Mock::new(vec![reply]).await;
    let mut pending = Pending::new();
    tokio::join!(refresh(&fixture, &mock, &mut pending), async {
        mock.wait_count(1).await;
        let snapshot = fixture.runtime.route_config().snapshot();
        let mut value = serde_json::to_value(snapshot.document()).unwrap();
        let provider = &mut value["providers"][0];
        provider["extra_body"]["profileArn"] = json!(PROFILE.replace("synthetic", "other"));
        provider["credentials"][0]["kiro_oauth"]["runtime_target"]["profile_arn"] =
            json!(PROFILE.replace("synthetic", "other"));
        fixture
            .runtime
            .commit_document(
                snapshot.revision().id(),
                serde_json::from_value(value).unwrap(),
                None,
            )
            .await
            .unwrap();
    });
    assert!(!old.is_active());
    assert_eq!(fixture.saved().api_key.as_deref(), Some("old-access"));
    refresh(&fixture, &mock, &mut pending).await;
    assert_eq!(mock.count(), 1);
    mock.stop().await;
}
