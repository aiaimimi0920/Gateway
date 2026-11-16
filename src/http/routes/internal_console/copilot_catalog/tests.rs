use super::*;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use std::{path::PathBuf, time::Duration};
use tower::ServiceExt;
#[path = "../../../../../tests/console_contract_support/mod.rs"]
mod support;
use support::{grant_console_secret_access, parse_json, ConsoleStateFixture, MANAGEMENT_TOKEN};
#[path = "saved_tests.rs"]
mod saved;

fn fixture() -> ConsoleStateFixture {
    ConsoleStateFixture::new(
        support::document("unrelated", "model", "unrelated-secret"),
        true,
    )
}

async fn send(router: Router, body: Value, grant: Option<&str>, management: bool) -> Response {
    let mut request = Request::post(PATH).header("content-type", "application/json");
    if management {
        request = request.header("x-management-token", MANAGEMENT_TOKEN);
    }
    if let Some(grant) = grant {
        request = request.header("x-secret-grant", grant);
    }
    router
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
}

fn body(f: &ConsoleStateFixture, token: &str) -> Value {
    json!({"githubToken":token,"expectedRevision":f.state.route_config.snapshot().revision().id()})
}

#[tokio::test]
async fn copilot_catalog_management_admission_and_no_store() {
    let f = fixture();
    let app = mount_with_settings(Router::new(), None).with_state(f.state.clone());
    let grant = grant_console_secret_access(&f.state).await;
    for (input, grant, auth, expected) in [
        (body(&f, "catalog-a"), None, false, StatusCode::UNAUTHORIZED),
        (body(&f, "catalog-a"), None, true, StatusCode::FORBIDDEN),
        (
            json!({"githubToken":"catalog-a","expectedRevision":"stale"}),
            Some(grant.as_str()),
            true,
            StatusCode::CONFLICT,
        ),
        (
            body(&f, " \n"),
            Some(grant.as_str()),
            true,
            StatusCode::BAD_REQUEST,
        ),
        (
            body(&f, "catalog-a"),
            Some(grant.as_str()),
            true,
            StatusCode::SERVICE_UNAVAILABLE,
        ),
    ] {
        let response = send(app.clone(), input, grant, auth).await;
        assert_eq!(response.status(), expected);
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert!(!parse_json(response).await.to_string().contains("catalog-a"));
    }
    let mut input = body(&f, "catalog-a");
    input["executable"] = json!("untrusted.exe");
    let response = send(app, input, Some(&grant), true).await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(response.headers()["cache-control"], "no-store");
}

fn settings() -> Settings {
    let root =
        PathBuf::from(std::env::var_os("GATEWAY_PROBE_ROOT").expect("explicit isolated test root"))
            .join(format!("management-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    Settings {
        executable: std::env::var_os("GATEWAY_COPILOT_TEST_EXE")
            .expect("explicit synthetic executable")
            .into(),
        workspace_root: root,
    }
}

#[tokio::test]
#[ignore = "requires the repository synthetic-runtime EXE and isolated GATEWAY_PROBE_ROOT"]
async fn copilot_catalog_management_reads_without_persisting_or_generating() {
    let f = fixture();
    let settings = settings();
    let app =
        mount_with_settings(Router::new(), Some(settings.clone())).with_state(f.state.clone());
    let grant = grant_console_secret_access(&f.state).await;
    let original = serde_json::to_value(f.state.route_config.snapshot().document()).unwrap();
    for (token, id) in [("catalog-a", "model-a"), ("catalog-b", "model-b")] {
        let response = send(app.clone(), body(&f, token), Some(&grant), true).await;
        let status = response.status();
        assert_eq!(response.headers()["cache-control"], "no-store");
        let value = parse_json(response).await;
        assert_eq!(status, StatusCode::OK, "{value}");
        assert_eq!(value["catalog"]["models"][0]["id"], id);
        assert!(!value.to_string().contains(token));
    }
    let response = send(app, body(&f, "catalog-denied"), Some(&grant), true).await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(!parse_json(response)
        .await
        .to_string()
        .contains("catalog-denied"));
    // Creating the real secret grant also starts the router's SiteHealth worker.
    f.state.shutdown.request("synthetic test completed");
    tokio::time::timeout(
        Duration::from_secs(45),
        f.state.lifecycle.background_tasks.join(),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        std::fs::read_dir(settings.workspace_root).unwrap().count(),
        0
    );
    assert_eq!(
        serde_json::to_value(f.state.route_config.snapshot().document()).unwrap(),
        original
    );
}

#[tokio::test]
#[ignore = "requires the repository synthetic-runtime EXE and isolated GATEWAY_PROBE_ROOT"]
async fn copilot_catalog_management_disconnect_and_shutdown_join_child() {
    for shutdown in [false, true] {
        let f = fixture();
        let settings = settings();
        let app =
            mount_with_settings(Router::new(), Some(settings.clone())).with_state(f.state.clone());
        let grant = grant_console_secret_access(&f.state).await;
        let input = body(&f, "catalog-hang");
        let task_app = app.clone();
        let task_grant = grant.clone();
        let task =
            tokio::spawn(async move { send(task_app, input, Some(&task_grant), true).await });
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let sent = std::fs::read_dir(&settings.workspace_root)
                    .unwrap()
                    .filter_map(Result::ok)
                    .any(|p| {
                        std::fs::read_to_string(p.path().join("rpc.jsonl"))
                            .is_ok_and(|s| s.contains("models.list"))
                    });
                if sent {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let busy = send(app, body(&f, "catalog-b"), Some(&grant), true).await;
        assert_eq!(busy.status(), StatusCode::CONFLICT);
        if shutdown {
            f.state.shutdown.request("synthetic shutdown");
            assert_eq!(
                task.await.unwrap().status(),
                StatusCode::SERVICE_UNAVAILABLE
            );
        } else {
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
            // Prove disconnect alone cleans the operation before stopping unrelated workers.
            tokio::time::timeout(Duration::from_secs(45), async {
                while std::fs::read_dir(&settings.workspace_root).unwrap().count() != 0 {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
            f.state
                .shutdown
                .request("synthetic disconnect cleanup completed");
        }
        tokio::time::timeout(
            Duration::from_secs(45),
            f.state.lifecycle.background_tasks.join(),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(
            std::fs::read_dir(settings.workspace_root).unwrap().count(),
            0
        );
    }
}
