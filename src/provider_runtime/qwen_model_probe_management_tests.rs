//! Real management auth, saved/temporary/named plans, exact wire and durable attribution.
use super::*;
use crate::{db, local_runtime::LocalRuntime};

fn policy(prompt: &str) -> serde_json::Value {
    json!({"automaticEnabled":false,"intervalMinutes":15,"modelSelection":"selected","models":["probe-alias"],
        "cases":[{"id":"ok","name":"OK","prompt":prompt,"expectedAnswer":"OK","difficulty":1,"enabled":true}]})
}

#[test]
fn qwen_probe_management_single_and_all_plan_sources_share_exact_sender_and_audit() {
    run(plans);
}
async fn plans() {
    let server = wire::Server::start().await;
    let mut doc = document();
    doc["providers"][0]["credentials"][3]["test_policy"] = policy("saved");
    doc["providers"][0]["test_plans"] = json!([{"id":"named", "name":"Named", "scopes":[{"kind":"account","id":"other"}], "policy":policy("named")}]);
    let mut fixture = ConsoleStateFixture::new(serde_json::from_value(doc).unwrap(), false);
    let directory = std::env::temp_dir().join(format!("qwen-probe-{}", uuid::Uuid::new_v4()));
    let local = LocalRuntime::open(&directory).await.unwrap();
    {
        let state = Arc::get_mut(&mut fixture.state).unwrap();
        state.upstream_client.http = server.client();
        state.local_runtime = Some(local.clone());
    }
    let grant = grant_console_secret_access(&fixture.state).await;
    let result = with_transport(
        &server.origin,
        send_console_json(
            &fixture.state,
            Method::POST,
            ENDPOINT,
            json!({"prompt":"single", "model":"probe-alias", "credentialIds":["other"]}),
            Some(&grant),
            None,
        ),
    )
    .await;
    assert_eq!(result.status(), StatusCode::OK);
    let result = parse_json(result).await;
    assert_eq!(
        result["result"]["results"][0]["status"], "passed",
        "{result}"
    );
    assert!(result["result"]["results"][0]["probePoint"]
        .as_str()
        .unwrap()
        .contains("workspace-b"));
    for (body, source) in [
        (
            json!({"testPlan":policy("temporary"), "scope":{"kind":"account","id":"other"}, "credentialIds":["other"]}),
            "account:temporary",
        ),
        (
            json!({"planId":"named", "credentialIds":["other"]}),
            "plan:named",
        ),
    ] {
        let response = with_transport(
            &server.origin,
            send_console_json(
                &fixture.state,
                Method::POST,
                ENDPOINT,
                body,
                Some(&grant),
                None,
            ),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = parse_json(response).await;
        let result = &body["result"]["results"][0];
        assert_eq!(result["status"], "passed", "{body}");
        assert_eq!(result["assessment"]["policySource"], source);
        assert_eq!(result["assessment"]["cases"][0]["correct"], true);
        assert!(!body.to_string().contains("synthetic-"));
    }
    let body = parse_json(
        with_transport(
            &server.origin,
            support::probe_credential(&fixture.state, &grant, "other"),
        )
        .await,
    )
    .await;
    assert_eq!(body["result"]["status"], "passed", "{body}");
    assert_eq!(body["result"]["assessment"]["policySource"], "account");
    let calls = server.calls();
    assert_eq!(calls.len(), 4);
    assert_eq!(server.attempts(), 4);
    for call in calls {
        assert_eq!(
            call.uri,
            "/workspace-b.ap-southeast-1.maas.aliyuncs.com/compatible-mode/v1/chat/completions"
        );
        assert_eq!(call.authorization, "Bearer synthetic-other-key");
        assert_eq!(call.body["model"], "qwen-synthetic");
        assert_eq!(call.body["stream"], false);
        assert_eq!(call.body["max_tokens"], 256);
        assert_eq!(call.body["n"], 1);
        assert!(call.user_agent.contains("codex"));
        assert!(call.body.get("input").is_none());
    }
    let audits = local
        .list_audits(&db::RequestAuditFilters::default())
        .await
        .unwrap();
    let summary = db::request_audits::summarize_request_audit_rows(&audits);
    assert_eq!(summary.total_requests, 4);
    assert_eq!(summary.completed_count, 4);
    assert_eq!(summary.credentials.len(), 1);
    assert_eq!(summary.credentials[0].credential_ref, "other");
    assert!(audits.iter().all(|a| a.prompt_tokens == Some(8)
        && a.completion_tokens == Some(1)
        && a.total_tokens == Some(9)));
    let states = local
        .list_model_states(db::CredentialModelStateFilters::default())
        .await
        .unwrap();
    assert!(states
        .iter()
        .any(|s| s.provider_credential_ref.as_deref() == Some("other")
            && s.model == "qwen-synthetic"
            && s.status == "active"));
    assert!(!states
        .iter()
        .any(|s| s.provider_credential_ref.as_deref() == Some("exact")));
    server.stop().await;
    drop(fixture);
    local.close().await;
}

#[test]
fn qwen_probe_auth_scope_disabled_and_model_rejection_do_not_touch_transport() {
    run(rejected);
}
async fn rejected() {
    let server = wire::Server::start().await;
    let mut fixture = fixture();
    Arc::get_mut(&mut fixture.state)
        .unwrap()
        .upstream_client
        .http = server.client();
    let grant = grant_console_secret_access(&fixture.state).await;
    for (body, grant, expected) in [
        (
            json!({"prompt":"OK", "model":"probe-alias", "credentialIds":["exact"]}),
            None,
            StatusCode::FORBIDDEN,
        ),
        (
            json!({"prompt":"OK", "model":"probe-alias", "credentialIds":["foreign"]}),
            Some(grant.as_str()),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"prompt":"OK", "model":"probe-alias", "credentialIds":["disabled"]}),
            Some(grant.as_str()),
            StatusCode::OK,
        ),
        (
            json!({"prompt":"OK", "model":"not-declared", "credentialIds":["exact"]}),
            Some(grant.as_str()),
            StatusCode::OK,
        ),
    ] {
        let response = with_transport(
            &server.origin,
            send_console_json(&fixture.state, Method::POST, ENDPOINT, body, grant, None),
        )
        .await;
        assert_eq!(response.status(), expected);
        let body = parse_json(response).await;
        if expected == StatusCode::OK {
            assert_eq!(
                body["result"]["results"][0]["status"], "unsupported",
                "{body}"
            );
        }
    }
    assert_eq!(server.attempts(), 0);
    server.stop().await;
}
