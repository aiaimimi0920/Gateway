//! Real transport and console authorization checks for explicit post-request task reads.
use super::{
    pipeline_fixture::{support::*, *},
    task_lookup::execute,
    test_support::*,
};
use axum::http::{Method, StatusCode};
use serde_json::{json, Value};
use std::time::Duration;
const PATH: &str = "/v1/internal/gateway/console/luma-agents-task";

#[tokio::test]
async fn task_lookup_reads_once_preserving_unknown_zero_and_actual_credits() {
    let jobs: Vec<_> = [
        Value::Null,
        json!({"credits_charged":null}),
        json!({"credits_charged":0}),
        json!({"credits_charged":12.5}),
    ]
    .into_iter()
    .map(|usage| {
        let mut item = job("completed");
        item["usage"] = usage;
        item
    })
    .collect();
    let server = Server::start(jobs.iter().cloned().map(|v| Reply::json(200, v)).collect()).await;
    for (index, expected) in [Value::Null, Value::Null, json!(0), json!(12.5)]
        .into_iter()
        .enumerate()
    {
        let result = within(&server.origin, execute(&server.client(), &payload(), ID))
            .await
            .unwrap();
        assert_eq!(result["usage"]["credits_charged"], expected);
        assert_eq!(result["state"], "completed");
        assert_eq!(result.as_object().unwrap().len(), 5);
        assert!(!result.to_string().contains("private"));
        assert_eq!(server.calls().len(), index + 1);
    }
    for call in server.calls() {
        assert_eq!(call.method, "GET");
        assert_eq!(call.path, format!("/v1/generations/{ID}"));
        assert!(call.raw_body.is_empty());
        assert_eq!(
            call.headers["authorization"],
            "Bearer luma-api-synthetic-only"
        );
        for header in [
            "cookie",
            "x-api-key",
            "x-management-token",
            "x-secret-grant",
        ] {
            assert!(!call.headers.contains_key(header));
        }
    }
    server.stop().await;
}

#[tokio::test]
async fn task_lookup_rejects_bad_identity_state_usage_and_unsafe_material() {
    let mutations = [
        ("id", json!("a290f1ee-6c54-4b01-90e6-d701748f0851")),
        ("state", json!("invented")),
        ("usage", json!({"credits_charged":-1})),
        ("usage", json!({"credits_charged":"12"})),
        ("model", json!("bad\nmodel")),
        ("type", json!("other")),
        ("failure_reason", json!("private")),
    ];
    let server = Server::start(
        mutations
            .iter()
            .map(|(key, value)| {
                let mut item = job("completed");
                item[*key] = value.clone();
                Reply::json(200, item)
            })
            .collect(),
    )
    .await;
    for _ in mutations {
        assert!(
            within(&server.origin, execute(&server.client(), &payload(), ID))
                .await
                .is_err()
        );
    }
    let count = server.calls().len();
    for id in [
        "",
        "../foreign",
        "https://foreign.test",
        "d290f1ee6c544b0190e6d701748f0851",
    ] {
        assert!(
            within(&server.origin, execute(&server.client(), &payload(), id))
                .await
                .is_err()
        );
    }
    for field in ["base_url", "api_key", "adapter"] {
        let mut value = serde_json::to_value(payload()).unwrap();
        value[field] = json!("foreign");
        assert!(within(
            &server.origin,
            execute(
                &server.client(),
                &serde_json::from_value(value).unwrap(),
                ID
            )
        )
        .await
        .is_err());
    }
    assert_eq!(server.calls().len(), count);
    server.stop().await;
}

#[tokio::test]
async fn task_lookup_returns_nonterminal_and_failed_facts_without_assets_or_replay() {
    let server = Server::start(
        ["queued", "processing", "failed"]
            .into_iter()
            .map(|state| Reply::json(200, job(state)))
            .collect(),
    )
    .await;
    for state in ["queued", "processing", "failed"] {
        let value = within(&server.origin, execute(&server.client(), &payload(), ID))
            .await
            .unwrap();
        assert_eq!(value["state"], state);
        assert!(value["usage"]["credits_charged"].is_null());
    }
    assert_eq!(server.calls().len(), 3);
    server.stop().await;
}

#[tokio::test]
async fn task_lookup_does_not_retry_http_errors_and_drops_pending_body() {
    let mut held = Reply::json(200, job("completed"));
    held.hold = true;
    let server = Server::start(vec![
        Reply::json(401, json!({"secret":"private"})),
        Reply::json(429, json!({"secret":"private"})),
        held,
    ])
    .await;
    for _ in 0..2 {
        let error = within(&server.origin, execute(&server.client(), &payload(), ID))
            .await
            .unwrap_err();
        assert!(!error.message.contains("private"));
    }
    assert!(tokio::time::timeout(
        Duration::from_millis(100),
        within(&server.origin, execute(&server.client(), &payload(), ID))
    )
    .await
    .is_err());
    server.wait_closed().await;
    assert_eq!(server.calls().len(), 3);
    server.stop().await;
}

fn input(fixture: &Fixture) -> Value {
    json!({"providerId":"luma-pipeline","credentialId":"exact","expectedRevision":fixture.state.route_config.snapshot().revision().id(),"generationId":ID})
}
#[tokio::test]
async fn task_lookup_management_binds_account_and_does_not_rewrite_audit_or_balance() {
    let mut charged = job("completed");
    charged["usage"] = json!({"credits_charged":7});
    let server = Server::start(vec![Reply::json(200, charged)]).await;
    within(
        &server.origin,
        Box::pin(async {
            let fixture = Box::pin(Fixture::new(&server, 5)).await;
            let grant = Box::pin(grant_console_secret_access(&fixture.state)).await;
            let revision = fixture
                .state
                .route_config
                .snapshot()
                .revision()
                .id()
                .to_owned();
            let response = Box::pin(send_console_json(
                &fixture.state,
                Method::POST,
                PATH,
                input(&fixture),
                Some(&grant),
                None,
            ))
            .await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()["cache-control"], "no-store");
            let value = parse_json(response).await;
            assert_eq!(value["revision"], revision);
            assert_eq!(value["credentialId"], "exact");
            assert_eq!(value["result"]["usage"]["credits_charged"], 7);
            assert_eq!(
                server.calls()[0].headers["authorization"],
                format!("Bearer {KEY}")
            );
            assert_eq!(fixture.balance().await.remaining_messages, Some(2));
            assert_eq!(
                fixture.state.route_config.snapshot().revision().id(),
                revision
            );
            assert!(fixture
                .state
                .local_runtime
                .as_ref()
                .unwrap()
                .list_audits(&crate::db::RequestAuditFilters::default())
                .await
                .unwrap()
                .is_empty());
            fixture.finish().await;
        }),
    )
    .await;
    server.stop().await;
}

#[tokio::test]
async fn task_lookup_management_denies_ungranted_foreign_stale_disabled_or_invalid_reads() {
    let server = Server::start(vec![]).await;
    within(
        &server.origin,
        Box::pin(async {
            let fixture = Box::pin(Fixture::new(&server, 5)).await;
            let grant = Box::pin(grant_console_secret_access(&fixture.state)).await;
            for (field, value) in [
                ("credentialId", "disabled"),
                ("credentialId", "backup-exact"),
                ("providerId", "other"),
                ("expectedRevision", "stale"),
                ("generationId", "../foreign"),
                ("url", "https://foreign.test"),
            ] {
                let mut body = input(&fixture);
                body[field] = json!(value);
                let response = Box::pin(send_console_json(
                    &fixture.state,
                    Method::POST,
                    PATH,
                    body,
                    Some(&grant),
                    None,
                ))
                .await;
                assert!(response.status().is_client_error(), "{field}");
                assert_eq!(response.headers()["cache-control"], "no-store");
            }
            for (access, origin) in [
                (None, None),
                (Some(grant.as_str()), Some("https://foreign.test")),
            ] {
                let response = Box::pin(send_console_json(
                    &fixture.state,
                    Method::POST,
                    PATH,
                    input(&fixture),
                    access,
                    origin,
                ))
                .await;
                assert!(response.status().is_client_error());
                assert_eq!(response.headers()["cache-control"], "no-store");
            }
            assert!(server.calls().is_empty());
            fixture.finish().await;
        }),
    )
    .await;
    server.stop().await;
}
