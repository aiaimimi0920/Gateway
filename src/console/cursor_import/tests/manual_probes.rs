//! The management router must not confuse an excluded probe target with a missing account.
use super::*;

const POOL: &str = "/v1/internal/gateway/console/providers/cursor/probe";
const SINGLE: &str = "/v1/internal/gateway/console/credentials/on/probe";

fn fixture() -> support::ConsoleStateFixture {
    let mut doc = document();
    doc.providers[0].credentials = serde_json::from_value(json!([
        {"id":"on","enabled":true,"account_name":"private@example.test","auth_token":access(),
         "expires_at":"2099-01-01T00:00:00Z"},
        {"id":"off","enabled":false}
    ]))
    .unwrap();
    support::ConsoleStateFixture::new(doc, false)
}

async fn probe(
    state: &Arc<AppState>,
    endpoint: &str,
    body: Value,
    grant: Option<&str>,
    status: StatusCode,
) -> Value {
    let response =
        support::send_console_json(state, Method::POST, endpoint, body, grant, None).await;
    assert_eq!(response.status(), status);
    if status == StatusCode::OK {
        assert_eq!(response.headers()["cache-control"], "no-store");
    }
    let value = support::parse_json(response).await;
    for secret in [
        access().as_str(),
        "private@example.test",
        "synthetic-unrelated-secret",
    ] {
        assert!(!value.to_string().contains(secret));
    }
    value
}

fn row(result: &Value, id: &str, state: &str) {
    assert_eq!(result["credentialId"], id);
    assert_eq!(result["providerId"], "cursor");
    assert_eq!(result["status"], "unsupported");
    assert_eq!(result["probePoint"], "未执行");
    let msg = result["message"].as_str().unwrap();
    assert!(msg.contains(state) && msg.contains("未向 Cursor 发送请求"));
    assert!(result.get("healthRecovery").is_none() && result.get("assessment").is_none());
}

#[tokio::test]
async fn cursor_manual_probes_report_the_saved_accounts_without_execution() {
    let f = fixture();
    let grant = support::grant_console_secret_access(&f.state).await;
    let rev = f.state.route_config.snapshot();
    let single = probe(&f.state, SINGLE, Value::Null, Some(&grant), StatusCode::OK).await;
    row(&single["result"], "on", "已启用");
    let pool = probe(
        &f.state,
        POOL,
        json!({"prompt":"test"}),
        Some(&grant),
        StatusCode::OK,
    )
    .await;
    let result = &pool["result"];
    assert_eq!(result["status"], "unsupported");
    assert_eq!(result["totalCount"], 2);
    assert_eq!(result["passedCount"], 0);
    assert_eq!(result["failedCount"], 0);
    assert_eq!(result["unsupportedCount"], 2);
    row(&result["results"][0], "on", "已启用");
    row(&result["results"][1], "off", "已暂停");
    let subset = probe(
        &f.state,
        POOL,
        json!({"prompt":"test","credentialIds":["off"]}),
        Some(&grant),
        StatusCode::OK,
    )
    .await;
    assert_eq!(subset["result"]["unsupportedCount"], 1);
    row(&subset["result"]["results"][0], "off", "已暂停");
    assert_eq!(
        rev.revision().id(),
        f.state.route_config.snapshot().revision().id()
    );
    assert!(rev.select_credential_probe_target("on").is_none());
    assert!(rev
        .credential_probe_targets_for_provider("cursor")
        .unwrap()
        .is_empty());
    assert!(rev.scheduled_credential_probe_targets().is_empty());
}

#[tokio::test]
async fn cursor_manual_probes_preserve_grant_membership_and_unknown_id_errors() {
    let f = fixture();
    for path in [SINGLE, POOL] {
        let denied = probe(&f.state, path, Value::Null, None, StatusCode::FORBIDDEN).await;
        assert_eq!(denied["error"]["code"], "console_secret_access_required");
    }
    let grant = support::grant_console_secret_access(&f.state).await;
    let missing = probe(
        &f.state,
        &SINGLE.replace("/on/", "/absent/"),
        Value::Null,
        Some(&grant),
        StatusCode::NOT_FOUND,
    )
    .await;
    assert_eq!(missing["error"]["code"], "console_credential_not_found");
    let plan = json!({"modelSelection":"all","cases":[{"id":"case","name":"Test","prompt":"test","difficulty":1,"enabled":true}]});
    for (body, code) in [
        (
            json!({"prompt":"x","credentialIds":["on","unrelated::default"]}),
            "console_probe_credential_mismatch",
        ),
        (
            json!({"testPlan":plan,"scope":{"kind":"subpool","id":"absent"}}),
            "console_probe_scope_mismatch",
        ),
        (json!({"planId":"absent"}), "console_probe_plan_mismatch"),
        (json!({"credentialIds":[]}), "console_probe_input_invalid"),
    ] {
        let err = probe(&f.state, POOL, body, Some(&grant), StatusCode::BAD_REQUEST).await;
        assert_eq!(err["error"]["code"], code);
    }
}
