//! Opt-in crash/restart proof: real external PG/Redis, normal Server TCP, synthetic Luma only.
use super::*;
use crate::{
    config::GatewayStorageMode,
    console::{ConsoleConfig, ConsoleConfigValues},
    upstream::luma_agents::{pipeline_fixture::support, test_support::*},
};
use axum::middleware::{from_fn, Next};
use sqlx::types::Json;
use std::{path::Path, process::Command as StdCommand, sync::Arc, time::Duration};
use tokio::process::{Child, Command};

const PUBLIC_KEY: &str = "luma-restart-fixture-public-only";
const MODEL: &str = "luma-server-image";
const HOSTED_MODEL: &str = "luma-server-hosted-image";
const ROTATED_MODEL: &str = "luma-server-hosted-rotated";
const PROVIDER: &str = "luma-server-hosted-fixture";
const KEY: &str = "luma-api-synthetic-restart";
const CREDENTIAL: &str = "luma-server-hosted-exact";
const CHILD: &str = "luma_reconciliation::server_recovery_tests::luma_reconciliation_server_child";

#[tokio::test]
#[ignore = "run through tools/luma/check-postgres.ps1 -ServerRestart"]
async fn luma_reconciliation_server_restart_after_os_kill() {
    let pg = super::postgres_tests::fixture_pool().await;
    let root = std::path::PathBuf::from(std::env::var("GATEWAY_LUMA_TEST_PROCESS_ROOT").unwrap());
    assert!(root.is_absolute());
    std::fs::create_dir(&root).unwrap();
    run_case(&pg, &root, MODEL).await;
    seed_hosted(&pg).await;
    run_case(&pg, &root, HOSTED_MODEL).await;
    run_case(&pg, &root, ROTATED_MODEL).await;
    pg.close().await;
}

async fn run_case(pg: &sqlx::PgPool, root: &Path, model: &'static str) {
    let root = root.join(model);
    std::fs::create_dir(&root).unwrap();
    let rotated = model == ROTATED_MODEL;
    let mut held = Reply::json(200, job("processing"));
    held.hold = true;
    let mut finished = job("completed");
    finished["usage"] = json!({"credits_charged":12.5});
    let mut queued = job("queued");
    queued["usage"] = json!({"credits_charged":0});
    let mut replies = vec![Reply::json(201, queued), held];
    if !rotated {
        replies.push(Reply::json(200, finished));
    }
    let peer = Server::start(replies).await;
    let mut generation = child(&root, &peer.origin, "generate", model);
    let first_pid = generation.id().unwrap();
    let address = ready(&root, &mut generation).await;
    let http = rquest::Client::builder().no_proxy().build().unwrap();
    let ready: Value = http
        .get(format!("{address}/readyz"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(ready["database"], true, "{ready}");
    assert_eq!(ready["redis"], true, "{ready}");
    let mut request = tokio::spawn(async move {
        http.post(format!("{address}/v1/images/generations"))
            .bearer_auth(PUBLIC_KEY)
            .json(&json!({"model":model,"prompt":"Synthetic process fixture"}))
            .send()
            .await
    });
    let audit_id = tokio::time::timeout(Duration::from_secs(12), async {
        loop {
            assert!(
                generation.try_wait().unwrap().is_none(),
                "generation Server exited early"
            );
            if request.is_finished() {
                panic!(
                    "generation must remain in progress at the crash point: {:?}",
                    (&mut request).await
                );
            }
            if peer.calls().len() == 2 {
                let (id, status, trace) = audit(pg, model).await;
                assert_eq!(status, "running");
                assert_eq!(trace["lumaAgentsGeneration"]["id"], ID);
                assert_eq!(trace["lumaAgentsGeneration"]["usage"]["credits_charged"], 0);
                assert_eq!(trace["lumaReconciliation"]["state"], "pending");
                if model != MODEL {
                    assert_eq!(trace["lumaReconciliation"]["version"], 2);
                    assert_eq!(
                        trace["lumaReconciliation"]["binding"]["providerId"],
                        PROVIDER
                    );
                    assert_eq!(
                        trace["lumaReconciliation"]["binding"]["credentialId"],
                        CREDENTIAL
                    );
                    assert!(!trace.to_string().contains(KEY));
                }
                return id;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("normal TCP generation never reached its durable checkpoint");
    // Force OS termination while the first poll is held. No shutdown/finalizer runs.
    generation.kill().await.unwrap();
    assert!(!generation.wait().await.unwrap().success());
    assert!(tokio::time::timeout(Duration::from_secs(5), request)
        .await
        .unwrap()
        .unwrap()
        .is_err());

    if rotated {
        sqlx::query("UPDATE gateway_provider_credentials SET payload_inline = $1 WHERE id = $2")
            .bind(Json(json!({"api_key":"luma-api-synthetic-replacement"})))
            .bind(CREDENTIAL)
            .execute(pg)
            .await
            .unwrap();
    }
    let mut recovery = child(&root, &peer.origin, "reconcile", model);
    let recovery_pid = recovery.id().unwrap();
    assert_ne!(first_pid, recovery_pid);
    assert!(
        tokio::time::timeout(Duration::from_secs(30), recovery.wait())
            .await
            .expect("recovery child did not exit")
            .unwrap()
            .success()
    );
    let (id, status, trace) = audit(pg, model).await;
    assert_eq!(id, audit_id);
    assert_eq!(
        status, "running",
        "booking credits must not rewrite the original outcome"
    );
    assert_eq!(trace["lumaAgentsGeneration"]["state"], "queued");
    assert_eq!(trace["lumaAgentsGeneration"]["mediaVerified"], false);
    assert_eq!(
        trace["lumaAgentsGeneration"]["usage"]["credits_charged"],
        if rotated { json!(0) } else { json!(12.5) }
    );
    assert_eq!(
        trace["lumaReconciliation"]["state"],
        if rotated { "blocked" } else { "settled" }
    );
    let calls = peer.calls();
    assert_eq!(
        calls.iter().map(|c| c.method.as_str()).collect::<Vec<_>>(),
        if rotated {
            vec!["POST", "GET"]
        } else {
            vec!["POST", "GET", "GET"]
        }
    );
    assert!(calls[1].path.ends_with(ID));
    assert_eq!(calls[0].headers["authorization"], format!("Bearer {KEY}"));
    if !rotated {
        assert_eq!(calls[1].path, calls[2].path);
        assert_eq!(
            calls[2].headers["authorization"],
            calls[0].headers["authorization"]
        );
    }
    peer.stop().await;
}

fn child(root: &Path, origin: &str, phase: &str, model: &str) -> Child {
    let mut native = StdCommand::new(std::env::current_exe().unwrap());
    native
        .args(["--ignored", "--exact", CHILD, "--nocapture"])
        .env("GATEWAY_LUMA_TEST_CHILD_PHASE", phase)
        .env("GATEWAY_LUMA_TEST_CHILD_ORIGIN", origin)
        .env("GATEWAY_LUMA_TEST_CHILD_MODEL", model)
        .env("GATEWAY_LUMA_TEST_PROCESS_ROOT", root);
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut native, 0x08000000); // CREATE_NO_WINDOW
    Command::from(native).kill_on_drop(true).spawn().unwrap()
}

async fn ready(root: &Path, child: &mut Child) -> String {
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            assert!(child.try_wait().unwrap().is_none(), "Server did not start");
            if let Ok(link) = tokio::fs::read_to_string(root.join("ready")).await {
                if let Ok(url) = url::Url::parse(&link) {
                    assert_eq!(url.host_str(), Some("127.0.0.1"));
                    assert!(url.port().is_some());
                    return link;
                }
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("Server readiness file not found")
}

async fn audit(pg: &sqlx::PgPool, model: &str) -> (String, String, Value) {
    let (id, status, trace): (String, String, Option<Json<Value>>) = sqlx::query_as(
        "SELECT id,status,route_trace FROM gateway_request_audits WHERE requested_model=$1",
    )
    .bind(model)
    .fetch_one(pg)
    .await
    .unwrap();
    let trace =
        trace.unwrap_or_else(|| panic!("missing durable route trace for {model} ({status})"));
    (id, status, trace.0)
}

#[tokio::test]
#[ignore = "child entry; exact invocation by the guarded Server restart contract only"]
async fn luma_reconciliation_server_child() {
    let phase = std::env::var("GATEWAY_LUMA_TEST_CHILD_PHASE").unwrap();
    let origin = std::env::var("GATEWAY_LUMA_TEST_CHILD_ORIGIN").unwrap();
    let root = std::path::PathBuf::from(std::env::var("GATEWAY_LUMA_TEST_PROCESS_ROOT").unwrap());
    let pg = super::postgres_tests::fixture_pool().await;
    let model = std::env::var("GATEWAY_LUMA_TEST_CHILD_MODEL").unwrap();
    let state = Box::pin(server_state(&root, &origin, &phase, &model)).await;
    if phase == "generate" {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        std::fs::write(
            root.join("ready"),
            format!("http://{}", listener.local_addr().unwrap()),
        )
        .unwrap();
        let app =
            crate::http::router::build_router(state).layer(from_fn(move |request, next: Next| {
                let origin = origin.clone();
                async move { within(&origin, next.run(request)).await }
            }));
        axum::serve(listener, app).await.unwrap();
    } else {
        assert_eq!(phase, "reconcile");
        let (_, _, trace) = audit(&pg, &model).await;
        let expires = trace["lumaReconciliation"]["activeUntil"].as_i64().unwrap();
        let store = store::Store::from_state(&state).unwrap();
        assert!(store.due(expires - 1).await.unwrap().is_empty());
        // Advance only the worker's supplied clock; no fixture SQL rewrites audit state.
        within(&origin, worker::tick(&state, &store, expires + 1))
            .await
            .unwrap();
        assert!(store.due(expires + 2).await.unwrap().is_empty());
        state.pg_pool.as_ref().unwrap().close().await;
    }
    pg.close().await;
}

async fn server_state(root: &Path, origin: &str, phase: &str, model: &str) -> Arc<AppState> {
    let redis = std::env::var("GATEWAY_LUMA_TEST_REDIS_URL").unwrap();
    let url = url::Url::parse(&redis).unwrap();
    assert_eq!(url.host_str(), Some("127.0.0.1"));
    assert!(matches!(url.port(), Some(port) if port != 6379));
    let console = ConsoleConfig::from_values(ConsoleConfigValues {
        routes_file: Some(root.join("routes.yaml")),
        state_dir: Some(root.join("state")),
        redis_namespace: Some(format!(
            "luma-restart-{}",
            std::env::var("GATEWAY_LUMA_TEST_RUN_ID").unwrap()
        )),
        ..Default::default()
    })
    .unwrap();
    if phase == "generate" {
        let document = if model != MODEL {
            json!({"providers":[],"model_routes":[]})
        } else {
            json!({"providers":[{"id":"luma-server-fixture","preset":"lumalabs-api",
            "base_url":crate::protocol::luma_agents::BASE,"explicit_credentials_only":true,
            "request_wait":{"mode":"disabled"},"supported_models":["uni-1"],
            "model_map":{MODEL:"uni-1"},"credentials":[{"id":"exact","api_key":KEY,"supported_models":["uni-1"]}]}],
            "model_routes":[{"pattern":MODEL,"provider_ids":["luma-server-fixture"]}]})
        };
        std::fs::write(
            &console.routes_file,
            serde_yaml::to_string(&document).unwrap(),
        )
        .unwrap();
    }
    let mut config = support::test_config();
    config.console = console;
    config.storage_mode = GatewayStorageMode::Server;
    config.database_url = Some(std::env::var("GATEWAY_LUMA_TEST_DATABASE_URL").unwrap());
    config.redis_url = redis;
    config.upstream_timeout_secs = 45;
    config.default_project_id = "platform-default-project".into();
    config.gateway_api_key = Some(PUBLIC_KEY.into());
    config.gateway_api_key_secret =
        (model != MODEL).then(|| "fixture-only-signing-secret-never-used-in-production".into());
    let mut state = Box::pin(crate::runtime::build_app_state(config))
        .await
        .unwrap();
    assert!(state.local_runtime.is_none() && state.pg_pool.is_some());
    Arc::get_mut(&mut state).unwrap().upstream_client.http = loopback_client(origin);
    state
}

async fn seed_hosted(pg: &sqlx::PgPool) {
    use crate::protocol::luma_agents::{ADAPTER, BASE, FAMILY, PROFILE};
    sqlx::query("INSERT INTO gateway_provider_accounts
        (id,label,adapter,protocol_family,protocol_profile,status,payload_inline,storage_mode,
        service_provider_key,service_provider_label,created_at,updated_at)
        VALUES ($1,'Synthetic hosted Luma',$2,$3,$4,'active',$5,'inline','luma-fixture','Luma fixture',now(),now())")
        .bind(PROVIDER).bind(ADAPTER).bind(FAMILY).bind(PROFILE)
        .bind(Json(json!({"adapter":ADAPTER,"base_url":BASE,"api_key":"", "default_model":"uni-1"})))
        .execute(pg).await.unwrap();
    sqlx::query(
        "INSERT INTO gateway_provider_credentials
        (id,provider_account_id,label,status,payload_inline,storage_mode,created_at,updated_at)
        VALUES ($1,$2,'Exact fixture key','active',$3,'inline',now(),now())",
    )
    .bind(CREDENTIAL)
    .bind(PROVIDER)
    .bind(Json(json!({"api_key":KEY})))
    .execute(pg)
    .await
    .unwrap();
    for model in [HOSTED_MODEL, ROTATED_MODEL] {
        sqlx::query(
            "INSERT INTO gateway_model_aliases
            (id,alias,provider_account_id,upstream_model,created_at,updated_at)
            VALUES ($1,$1,$2,'uni-1',now(),now())",
        )
        .bind(model)
        .bind(PROVIDER)
        .execute(pg)
        .await
        .unwrap();
    }
}
