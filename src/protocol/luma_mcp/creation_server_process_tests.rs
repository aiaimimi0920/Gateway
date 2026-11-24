//! Guarded TCP crash proof for durable run IDs and interruptions before a receipt.
use super::{pool, state, tools, SIGNING};
use crate::protocol::luma_mcp::creation_tests::{
    json, opened, progress, replies, started, support, test_support, usage, Peer, Value,
};
use axum::middleware::{from_fn, Next};
use std::{
    path::{Path, PathBuf},
    process::Command as StdCommand,
    time::Duration,
};
use tokio::{
    process::{Child, Command},
    time::{sleep, timeout},
};

const CHILD: &str =
    "protocol::luma_mcp::creation_tests::server_tests::process_tests::luma_mcp_process_child";
const TOKEN: &str = support::MANAGEMENT_TOKEN;
const CONSOLE: &str = "/v1/internal/gateway/console";
const POLL: Duration = Duration::from_millis(100);

#[tokio::test]
#[ignore = "run through tools/luma/check-postgres.ps1 -McpRestart"]
async fn luma_mcp_server_restart_after_os_kill() {
    let pg = pool().await;
    let root = PathBuf::from(std::env::var_os("GATEWAY_LUMA_TEST_PROCESS_ROOT").unwrap());
    assert!(root.is_absolute());
    std::fs::create_dir(&root).unwrap();
    tokio::join!(run_case(&pg, &root, true), run_case(&pg, &root, false));
    pg.close().await;
}

async fn run_case(pg: &sqlx::PgPool, root: &Path, has_run: bool) {
    let name = if has_run {
        "durable-run"
    } else {
        "unconfirmed-submit"
    };
    let root = root.join(name);
    std::fs::create_dir(&root).unwrap();
    let mut wire = vec![opened(), started()];
    if has_run {
        wire.extend([
            progress("running", true, json!([])),
            progress("completed", false, json!([])),
        ]);
    }
    wire.push(usage(false));
    let mut script = replies(wire);
    let held = if has_run { 8 } else { 5 };
    script[held].hold = true;
    // The upstream saw the request but sends no JSON receipt before the crash.
    script[held].body.clear();
    let peer = Peer::new(script).await;
    let mut first = child(&root, &peer, "initial");
    let first_pid = first.id().unwrap();
    let initial = Client::connect(&root, &mut first, "initial").await;
    let prepared = initial.ok("prepare", None).await;
    let id = prepared["id"].as_str().unwrap().to_owned();
    assert_eq!(peer.count(), 0);
    if has_run {
        assert_eq!(initial.ok("start", Some(&id)).await["runId"], "r1");
    }
    let request = tokio::spawn({
        let id = id.clone();
        async move {
            initial
                .act(if has_run { "read" } else { "start" }, Some(&id))
                .await
        }
    });
    timeout(Duration::from_secs(15), async {
        while peer.count() <= held {
            assert!(
                !request.is_finished(),
                "Gateway returned before reaching the crash point"
            );
            assert!(first.try_wait().unwrap().is_none());
            sleep(POLL).await;
        }
    })
    .await
    .expect("MCP peer did not receive the held operation");
    let (checkpoint, version, active) = row(pg, &id).await;
    assert!(active && !request.is_finished());
    assert_eq!(
        checkpoint["state"],
        if has_run { "running" } else { "submitting" }
    );
    assert_eq!(checkpoint["sessionId"], "s1");
    assert_eq!(
        checkpoint["runId"],
        if has_run { json!("r1") } else { Value::Null }
    );
    assert!(checkpoint["usage"].is_null());
    first.kill().await.unwrap();
    assert!(!first.wait().await.unwrap().success());
    assert!(timeout(Duration::from_secs(5), request)
        .await
        .unwrap()
        .unwrap()
        .is_err());

    let mut second = child(&root, &peer, "reopened");
    let second_pid = second.id().unwrap();
    assert_ne!(first_pid, second_pid);
    let reopened = Client::connect(&root, &mut second, "reopened").await;
    assert_eq!(persisted(reopened.ok("read", Some(&id)).await), checkpoint);
    assert_eq!(row(pg, &id).await, (checkpoint, version, true));
    assert_eq!(peer.count(), held + 1);
    // Observe the real PostgreSQL clock. Neither the test nor the replacement process rewrites the lease.
    timeout(Duration::from_secs(95), async {
        while row(pg, &id).await.2 {
            sleep(Duration::from_secs(1)).await;
        }
    })
    .await
    .expect("natural PostgreSQL lease expiry was not observed");
    assert_eq!(row(pg, &id).await.1, version);
    let task = reopened.ok("read", Some(&id)).await;
    assert_eq!(task["id"], id);
    assert_eq!(task["sessionId"], "s1");
    assert_eq!(task["usage"]["credits"], 9);
    assert_eq!(task["usage"]["settled"], has_run);
    if has_run {
        assert_eq!(task["state"], "completed");
        assert_eq!(task["runId"], "r1");
        assert_eq!(
            tools(&peer),
            [
                "luma_session_open",
                "luma_generate_image",
                "luma_run_await",
                "luma_run_await",
                "luma_usage"
            ]
        );
        let calls = peer.calls.lock().unwrap();
        for index in [8, 11] {
            assert_eq!(calls[index].1["params"]["arguments"]["run_id"], "r1");
        }
    } else {
        assert_eq!(task["state"], "unconfirmed");
        assert!(task["runId"].is_null());
        assert_eq!(
            tools(&peer),
            ["luma_session_open", "luma_generate_image", "luma_usage"]
        );
    }
    assert_eq!(reopened.act("start", Some(&id)).await.unwrap().0, 409);
    assert_eq!(peer.count(), if has_run { 15 } else { 9 });
    let (saved, latest, active) = row(pg, &id).await;
    assert_eq!(saved, persisted(task.clone()));
    assert!(latest > version && !active);
    assert!(peer
        .calls
        .lock()
        .unwrap()
        .iter()
        .all(
            |(headers, _)| headers["authorization"] == "Bearer synthetic-access"
                && !headers.contains_key("x-secret-grant")
        ));
    std::fs::write(root.join("stop"), b"").unwrap();
    assert!(timeout(Duration::from_secs(10), second.wait())
        .await
        .unwrap()
        .unwrap()
        .success());
    peer.stop().await;
    println!("{name}: killed pid {first_pid}; recovered via pid {second_pid} after natural PG lease expiry; final state={}", task["state"]);
}

fn persisted(mut task: Value) -> Value {
    let object = task.as_object_mut().unwrap();
    object.remove("version");
    object.remove("leaseUntil");
    task
}

async fn row(pg: &sqlx::PgPool, id: &str) -> (Value, i64, bool) {
    sqlx::query_as("SELECT payload, version, lease_until > floor(extract(epoch FROM clock_timestamp()))::bigint FROM gateway_luma_mcp_creation_tasks WHERE id = $1")
        .bind(id).fetch_one(pg).await.unwrap()
}

struct Client {
    http: rquest::Client,
    origin: String,
    revision: String,
    grant: String,
}

impl Client {
    async fn connect(root: &Path, child: &mut Child, phase: &str) -> Self {
        let path = root.join(format!("ready-{phase}.json"));
        let info: Value = timeout(Duration::from_secs(20), async {
            loop {
                assert!(child.try_wait().unwrap().is_none(), "Server did not start");
                if let Ok(bytes) = std::fs::read(&path) {
                    if let Ok(parsed) = serde_json::from_slice(&bytes) {
                        break parsed;
                    }
                }
                sleep(POLL).await;
            }
        })
        .await
        .expect("Server readiness file not found");
        let origin = info["origin"].as_str().unwrap();
        let url = url::Url::parse(origin).unwrap();
        assert_eq!((url.scheme(), url.host_str()), ("http", Some("127.0.0.1")));
        assert!(url.port().is_some());
        let http = rquest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(25))
            .build()
            .unwrap();
        let ready: Value = http
            .get(format!("{origin}/readyz"))
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
        let grant: Value = http
            .post(format!("{origin}{CONSOLE}/session/confirm-secret-access"))
            .header("x-management-token", TOKEN)
            .json(&json!({"token":TOKEN}))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        Self {
            http,
            origin: origin.into(),
            revision: info["revision"].as_str().unwrap().into(),
            grant: grant["grant"].as_str().unwrap().into(),
        }
    }

    async fn act(&self, action: &str, id: Option<&str>) -> Result<(u16, Value), rquest::Error> {
        let mut command = json!({"providerId":"luma-mcp", "credentialId":"saved-mcp", "expectedRevision":self.revision, "action":action});
        if let Some(id) = id {
            command["taskId"] = json!(id);
        }
        if matches!(action, "prepare" | "start") {
            command["request"] =
                json!({"kind":"image", "prompt":"a blue circle", "budgetCredits":20});
            command["chargeConfirmed"] = json!(action == "start");
        }
        let response = self
            .http
            .post(format!("{}{CONSOLE}/luma-mcp-creation", self.origin))
            .header("x-management-token", TOKEN)
            .header("x-secret-grant", &self.grant)
            .json(&command)
            .send()
            .await?;
        Ok((response.status().as_u16(), response.json().await?))
    }

    async fn ok(&self, action: &str, id: Option<&str>) -> Value {
        let (status, value) = self.act(action, id).await.unwrap();
        assert_eq!(status, 200, "{action}: {value}");
        value["task"].clone()
    }
}

fn child(root: &Path, peer: &Peer, phase: &str) -> Child {
    let mut native = StdCommand::new(std::env::current_exe().unwrap());
    native
        .args(["--ignored", "--exact", CHILD, "--nocapture"])
        .env("GATEWAY_LUMA_TEST_MCP_ROOT", root)
        .env("GATEWAY_LUMA_TEST_MCP_PEER", &peer.endpoint)
        .env("GATEWAY_LUMA_TEST_MCP_PHASE", phase);
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut native, 0x08000000); // CREATE_NO_WINDOW
    Command::from(native).kill_on_drop(true).spawn().unwrap()
}

#[tokio::test]
#[ignore = "child entry; exact invocation by the guarded MCP restart contract only"]
async fn luma_mcp_process_child() {
    pool().await.close().await;
    let root = PathBuf::from(std::env::var_os("GATEWAY_LUMA_TEST_MCP_ROOT").unwrap());
    let origin = std::env::var("GATEWAY_LUMA_TEST_MCP_PEER").unwrap();
    let phase = std::env::var("GATEWAY_LUMA_TEST_MCP_PHASE").unwrap();
    assert!(root.is_absolute() && matches!(phase.as_str(), "initial" | "reopened"));
    let state = state(&root, TOKEN, Some(SIGNING)).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    std::fs::write(
        root.join(format!("ready-{phase}.json")),
        json!({
            "origin":format!("http://{}", listener.local_addr().unwrap()),
            "revision":state.route_config.snapshot().revision().id()
        })
        .to_string(),
    )
    .unwrap();
    let app = crate::http::router::build_router(state.clone()).layer(from_fn(
        move |request, next: Next| test_support::scope(origin.clone(), next.run(request)),
    ));
    let stop = root.join("stop");
    timeout(Duration::from_secs(135), async {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .with_graceful_shutdown(async move {
            while !stop.is_file() {
                sleep(POLL).await;
            }
        })
        .await
        .unwrap();
    })
    .await
    .expect("parent did not stop the fixture");
    state.pg_pool.as_ref().unwrap().close().await;
}
