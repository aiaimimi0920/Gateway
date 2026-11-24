//! Opt-in browser acceptance for persisted cancelled facts and explicit single-task reads.
use super::{pipeline_fixture::*, test_support::*};
use axum::{extract::Request, middleware::Next};
use serde_json::{json, Value};
use std::{net::SocketAddr, path::PathBuf, time::Duration};

fn replies() -> Vec<Reply> {
    let mut held = Reply::json(200, job("processing"));
    held.hold = true;
    let mut values = vec![Reply::json(201, job("queued")), held];
    for credits in [Value::Null, json!(0), json!(12.5)] {
        let mut value = job("completed");
        value["usage"] = json!({"credits_charged":credits});
        values.push(Reply::json(200, value));
    }
    values.push(Reply::json(401, json!({"message":"supplier-private-body"})));
    values.push(Reply::json(429, json!({"message":"supplier-private-body"})));
    let mut foreign = job("completed");
    foreign["id"] = json!("d290f1ee-6c54-4b01-90e6-d701748f0852");
    values.push(Reply::json(200, foreign));
    values
}

async fn cancelled(fixture: &Fixture, server: &Server) -> Value {
    within(&server.origin, async {
        let mut call = Box::pin(fixture.open("images/generations", body(IMAGE)));
        tokio::time::timeout(Duration::from_secs(5), async {
            tokio::select! {
                _ = &mut call => panic!("fixture must remain pending"),
                _ = async {
                    while server.calls().len() < 2 { tokio::task::yield_now().await; }
                } => {}
            }
        })
        .await
        .unwrap();
        drop(call);
        let row = fixture.audit().await;
        assert_eq!(row.status, "cancelled");
        super::cancellation_audit_tests::assert_phase(&row, 1);
        server.wait_closed().await;
        serde_json::to_value(row).unwrap()
    })
    .await
}

#[tokio::test]
#[ignore = "Requires tools/luma/task-browser-check.mjs and a new explicit fixture root"]
async fn luma_agents_task_normal_main_browser_host() {
    tokio::time::timeout(Duration::from_secs(1800), Box::pin(serve()))
        .await
        .unwrap();
}

async fn serve() {
    let root = PathBuf::from(
        std::env::var_os("GATEWAY_LUMA_TASK_UI_ROOT").expect("Explicit fixture root required"),
    );
    assert!(
        root.is_absolute() && !root.exists(),
        "Use a new absolute fixture root"
    );
    std::fs::create_dir(&root).unwrap();
    let server = Server::start(replies()).await;
    let fixture = Box::pin(Fixture::new(&server, 5)).await;
    let original_audit = Box::pin(cancelled(&fixture, &server)).await;
    let revision = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_owned();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let origin = server.origin.clone();
    let app = crate::http::router::build_router(fixture.state.clone()).layer(
        axum::middleware::from_fn(move |request: Request, next: Next| {
            let origin = origin.clone();
            async move { within(&origin, next.run(request)).await }
        }),
    );
    let (shutdown, stopped) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(async {
            let _ = stopped.await;
        })
        .await
        .unwrap();
    });
    std::fs::write(
        root.join("ready.json"),
        serde_json::to_vec_pretty(&json!({
            "origin":format!("http://{address}"),"pid":std::process::id(),"syntheticOnly":true,
            "scenario":"luma-task-normal-main-v1","provider":"luma-pipeline","credential":"exact",
            "revision":revision,"taskId":ID,"auditId":original_audit["id"]
        }))
        .unwrap(),
    )
    .unwrap();
    // Browser publishes an atomic finish/abort file. The owner always closes before assertions.
    let command = tokio::time::timeout(Duration::from_secs(1700), async {
        loop {
            tokio::time::sleep(Duration::from_millis(200)).await;
            match std::fs::read(root.join("command.json")) {
                Ok(bytes) => break serde_json::from_slice::<Value>(&bytes),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(error) => panic!("Fixture command read failed: {error}"),
            }
        }
    })
    .await;
    shutdown.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .unwrap()
        .unwrap();
    let calls = server.calls();
    let balance = fixture.balance().await.remaining_messages;
    let final_revision = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_owned();
    let audits = fixture
        .state
        .local_runtime
        .as_ref()
        .unwrap()
        .list_audits(&crate::db::RequestAuditFilters::default())
        .await
        .unwrap();
    fixture.finish().await;
    server.stop().await;
    assert_eq!(
        command.unwrap().unwrap()["action"],
        "finish",
        "Browser did not complete"
    );
    assert_eq!(balance, Some(2));
    assert_eq!(revision, final_revision);
    assert_eq!(audits.len(), 1);
    assert_eq!(serde_json::to_value(&audits[0]).unwrap(), original_audit);
    assert_eq!(calls.len(), 8);
    for (index, call) in calls.iter().enumerate() {
        assert_eq!(call.method, if index == 0 { "POST" } else { "GET" });
        assert_eq!(
            call.path,
            if index == 0 {
                "/v1/generations".to_owned()
            } else {
                format!("/v1/generations/{ID}")
            }
        );
        assert_eq!(call.headers["authorization"], format!("Bearer {KEY}"));
        for name in ["cookie", "origin", "x-secret-grant", "x-management-token"] {
            assert!(!call.headers.contains_key(name));
        }
        if index > 0 {
            assert!(call.raw_body.is_empty());
        }
    }
    std::fs::write(root.join("completed.json"), serde_json::to_vec_pretty(&json!({
        "status":"passed","revisionUnchanged":true,"auditUnchanged":true,"auditCount":1,
        "ordinaryKeyBalance":balance,"seedCalls":2,"lookupCalls":6,
        "upstreamCalls":calls.iter().map(|c|json!({"method":c.method,"path":c.path})).collect::<Vec<_>>(),
        "scope":"normal main, real management TCP and local persisted cancellation; synthetic supplier only"
    })).unwrap()).unwrap();
}
