//! Guarded Server router -> independent PostgreSQL pools -> synthetic MCP transport.
use super::*;
use crate::{config::GatewayStorageMode, http::router::build_router, state::AppState};
use axum::{body::Body, http::Request};
use std::path::Path;
use tower::ServiceExt;

#[path = "creation_server_process_tests.rs"]
mod process_tests;

const OTHER: &str = "luma-test-other-management-token";
const SIGNING: &str = "luma-test-stable-signing-secret-not-a-production-key";

#[tokio::test]
#[ignore = "run through tools/luma/check-postgres.ps1 -McpCreation"]
async fn luma_mcp_creation_server_durable_owner() {
    let pg = pool().await;
    let root =
        std::path::PathBuf::from(std::env::var_os("GATEWAY_LUMA_TEST_PROCESS_ROOT").unwrap());
    let original = state(
        &root.join("first"),
        support::MANAGEMENT_TOKEN,
        Some(SIGNING),
    )
    .await;
    let peer = Peer::new(replies(vec![
        opened(),
        started(),
        progress("running", true, json!([])),
        usage(true),
    ]))
    .await;
    let a = grant(&original, support::MANAGEMENT_TOKEN).await;
    assert_eq!(
        send(
            &original,
            &peer,
            support::MANAGEMENT_TOKEN,
            "wrong-grant",
            "prepare",
            None
        )
        .await
        .0,
        403,
    );
    let (status, prepared) = send(
        &original,
        &peer,
        support::MANAGEMENT_TOKEN,
        &a,
        "prepare",
        None,
    )
    .await;
    assert_eq!(status, 200, "{prepared}");
    let id = prepared["task"]["id"].as_str().unwrap();
    assert!(peer.calls.lock().unwrap().is_empty());
    let second = state(
        &root.join("second"),
        support::MANAGEMENT_TOKEN,
        Some(SIGNING),
    )
    .await;
    let b = grant(&second, support::MANAGEMENT_TOKEN).await;
    assert_eq!(
        send(
            &second,
            &peer,
            support::MANAGEMENT_TOKEN,
            &a,
            "start",
            Some(id)
        )
        .await
        .0,
        403,
    );
    let (left, right) = tokio::join!(
        send(
            &original,
            &peer,
            support::MANAGEMENT_TOKEN,
            &a,
            "start",
            Some(id)
        ),
        send(
            &second,
            &peer,
            support::MANAGEMENT_TOKEN,
            &b,
            "start",
            Some(id)
        )
    );
    // A competing request can get a busy task view or lose the CAS. Exactly one run is opened.
    assert!([left.0, right.0].contains(&200), "{left:?} {right:?}");
    assert!(matches!(left.0, 200 | 409) && matches!(right.0, 200 | 409));
    assert_eq!(tools(&peer), ["luma_session_open", "luma_generate_image"]);
    original.pg_pool.as_ref().unwrap().close().await;
    second.pg_pool.as_ref().unwrap().close().await;
    let rotated = state(&root.join("rotated"), OTHER, Some(SIGNING)).await;
    let grant_other = grant(&rotated, OTHER).await;
    let (status, _) = send(&rotated, &peer, OTHER, &grant_other, "read", Some(id)).await;
    assert_eq!(status, 400);
    rotated.pg_pool.as_ref().unwrap().close().await;
    let reopened = state(
        &root.join("reopened"),
        support::MANAGEMENT_TOKEN,
        Some(SIGNING),
    )
    .await;
    let c = grant(&reopened, support::MANAGEMENT_TOKEN).await;
    let (status, task) = send(
        &reopened,
        &peer,
        support::MANAGEMENT_TOKEN,
        &c,
        "read",
        Some(id),
    )
    .await;
    assert_eq!(status, 200, "{task}");
    assert_eq!(task["task"]["runId"], "r1");
    assert_eq!(task["task"]["usage"]["credits"], 9);
    assert_eq!(
        send(
            &reopened,
            &peer,
            support::MANAGEMENT_TOKEN,
            &c,
            "start",
            Some(id)
        )
        .await
        .0,
        409
    );
    assert_eq!(
        tools(&peer),
        [
            "luma_session_open",
            "luma_generate_image",
            "luma_run_await",
            "luma_usage"
        ]
    );
    let (owner_bytes, payload, lease): (Vec<u8>, Value, i64) = sqlx::query_as(
        "SELECT owner_tag, payload, lease_until FROM gateway_luma_mcp_creation_tasks WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&pg)
    .await
    .unwrap();
    assert_eq!(owner_bytes.len(), 32);
    assert_eq!(lease, 0);
    assert_eq!(payload["runId"], "r1");
    assert_eq!(payload["usage"]["credits"], 9);
    assert!(payload.get("ownerTag").is_none());
    assert!(payload.get("leaseUntil").is_none() && payload.get("version").is_none());
    let json = payload.to_string();
    assert!(!json.contains("a blue circle") && !json.contains(SIGNING));
    assert!(!json.contains(support::MANAGEMENT_TOKEN));
    interrupted_submission(&reopened, &peer, &pg, &c).await;
    unavailable_journal(&root, &reopened, &peer, &pg, &c).await;
    let public_audits: i64 =
        sqlx::query_scalar("SELECT count(*) FROM gateway_request_audits WHERE id = $1")
            .bind(id)
            .fetch_one(&pg)
            .await
            .unwrap();
    assert_eq!(public_audits, 0);
    reopened.pg_pool.as_ref().unwrap().close().await;
    pg.close().await;
    peer.stop().await;
}

async fn interrupted_submission(
    state: &Arc<AppState>,
    peer: &Peer,
    pg: &sqlx::PgPool,
    grant: &str,
) {
    let token = support::MANAGEMENT_TOKEN;
    let (status, task) = send(state, peer, token, grant, "prepare", None).await;
    assert_eq!(status, 200, "{task}");
    let id = task["task"]["id"].as_str().unwrap();
    // Seed a durable pre-send checkpoint and active lease in the guarded, disposable database.
    sqlx::query("UPDATE gateway_luma_mcp_creation_tasks SET
        payload = jsonb_set(payload, '{state}', to_jsonb('submitting'::text)),
        version = version + 1, lease_until = floor(extract(epoch FROM clock_timestamp()))::bigint + 90
        WHERE id = $1")
        .bind(id).execute(pg).await.unwrap();
    let calls = peer.calls.lock().unwrap().len();
    let (status, task) = send(state, peer, token, grant, "read", Some(id)).await;
    assert_eq!(status, 200, "{task}");
    assert_eq!(task["task"]["state"], "submitting");
    sqlx::query("UPDATE gateway_luma_mcp_creation_tasks SET lease_until = 1 WHERE id = $1")
        .bind(id)
        .execute(pg)
        .await
        .unwrap();
    let (status, task) = send(state, peer, token, grant, "read", Some(id)).await;
    assert_eq!(status, 200, "{task}");
    assert_eq!(task["task"]["state"], "unconfirmed");
    assert_eq!(task["task"]["error"], "luma_mcp_operation_interrupted");
    assert_eq!(
        send(state, peer, token, grant, "start", Some(id)).await.0,
        409
    );
    assert_eq!(peer.calls.lock().unwrap().len(), calls);
}

async fn unavailable_journal(
    root: &Path,
    original: &Arc<AppState>,
    peer: &Peer,
    pg: &sqlx::PgPool,
    original_grant: &str,
) {
    let token = support::MANAGEMENT_TOKEN;
    let calls = peer.calls.lock().unwrap().len();
    let unsigned = state(&root.join("unsigned"), token, None).await;
    let grant = grant(&unsigned, token).await;
    let (status, body) = send(&unsigned, peer, token, &grant, "prepare", None).await;
    assert_eq!(status, 503, "{body}");
    assert!(body
        .to_string()
        .contains("luma_mcp_journal_signing_unavailable"));
    unsigned.pg_pool.as_ref().unwrap().close().await;
    // Rename, never delete. This connection may only reach the guarded fixture database.
    sqlx::query("ALTER TABLE gateway_luma_mcp_creation_tasks RENAME TO fixture_unapplied_journal")
        .execute(pg)
        .await
        .unwrap();
    let (status, body) = send(original, peer, token, original_grant, "prepare", None).await;
    sqlx::query("ALTER TABLE fixture_unapplied_journal RENAME TO gateway_luma_mcp_creation_tasks")
        .execute(pg)
        .await
        .unwrap();
    assert_eq!(status, 503, "{body}");
    assert!(body
        .to_string()
        .contains("luma_mcp_journal_schema_unavailable"));
    assert_eq!(peer.calls.lock().unwrap().len(), calls);
}

async fn pool() -> sqlx::PgPool {
    let address = std::env::var("GATEWAY_LUMA_TEST_DATABASE_URL").unwrap();
    let parsed = url::Url::parse(&address).unwrap();
    assert_eq!(parsed.host_str(), Some("127.0.0.1"));
    assert_eq!(parsed.path(), "/gateway_luma_fixture");
    assert!(matches!(parsed.port(), Some(port) if port != 5432));
    let pg = sqlx::PgPool::connect(&address).await.unwrap();
    let run: String = sqlx::query_scalar("SELECT run_id FROM luma_fixture_guard")
        .fetch_one(&pg)
        .await
        .unwrap();
    assert_eq!(run, std::env::var("GATEWAY_LUMA_TEST_RUN_ID").unwrap());
    pg
}

async fn state(root: &Path, token: &str, signing: Option<&str>) -> Arc<AppState> {
    std::fs::create_dir_all(root).unwrap();
    let redis = std::env::var("GATEWAY_LUMA_TEST_REDIS_URL").unwrap();
    let parsed = url::Url::parse(&redis).unwrap();
    assert_eq!(parsed.host_str(), Some("127.0.0.1"));
    assert!(matches!(parsed.port(), Some(port) if port != 6379));
    let mut config = support::test_config();
    config.storage_mode = GatewayStorageMode::Server;
    config.database_url = Some(std::env::var("GATEWAY_LUMA_TEST_DATABASE_URL").unwrap());
    config.redis_url = redis;
    config.gateway_api_key = None;
    config.gateway_api_key_secret = signing.map(str::to_owned);
    config.gateway_management_token = Some(token.into());
    config.console.state_dir = root.join("state");
    config.console.routes_file = root.join("routes.yaml");
    config.console.redis_namespace = format!(
        "luma-mcp-{}",
        std::env::var("GATEWAY_LUMA_TEST_RUN_ID").unwrap()
    );
    if !config.console.routes_file.exists() {
        browser_fixture::write_routes(&config.console.routes_file);
    }
    let state = Box::pin(crate::runtime::build_app_state(config))
        .await
        .unwrap();
    assert!(state.local_runtime.is_none() && state.pg_pool.is_some());
    state
}

async fn grant(state: &Arc<AppState>, token: &str) -> String {
    let (status, value) = post(
        state,
        token,
        None,
        "/v1/internal/gateway/console/session/confirm-secret-access",
        json!({"token":token}),
    )
    .await;
    assert_eq!(status, 200, "{value}");
    value["grant"].as_str().unwrap().into()
}

async fn send(
    state: &Arc<AppState>,
    peer: &Peer,
    token: &str,
    grant: &str,
    action: &str,
    id: Option<&str>,
) -> (u16, Value) {
    let mut body = json!({"providerId":"luma-mcp","credentialId":"saved-mcp",
        "expectedRevision":state.route_config.snapshot().revision().id(),"action":action});
    if let Some(id) = id {
        body["taskId"] = json!(id);
    }
    if matches!(action, "prepare" | "start") {
        body["request"] = json!({"kind":"image","prompt":"a blue circle","budgetCredits":20});
        body["chargeConfirmed"] = json!(action == "start");
    }
    test_support::scope(
        peer.endpoint.clone(),
        post(
            state,
            token,
            Some(grant),
            "/v1/internal/gateway/console/luma-mcp-creation",
            body,
        ),
    )
    .await
}

async fn post(
    state: &Arc<AppState>,
    token: &str,
    grant: Option<&str>,
    path: &str,
    body: Value,
) -> (u16, Value) {
    let mut request = Request::post(path)
        .header("x-management-token", token)
        .header("content-type", "application/json");
    if let Some(grant) = grant {
        request = request.header("x-secret-grant", grant);
    }
    let response = build_router(Arc::clone(state))
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    (
        response.status().as_u16(),
        support::parse_json(response).await,
    )
}

fn tools(peer: &Peer) -> Vec<String> {
    peer.calls
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, v)| v["method"] == "tools/call")
        .map(|(_, v)| v["params"]["name"].as_str().unwrap().to_owned())
        .collect()
}
