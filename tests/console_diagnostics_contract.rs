//! Browser/desktop share authenticated backend diagnostics, never client filesystem commands.
use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode},
};
use neuro_gateway::http::router::build_router;
use std::{fs, sync::Arc};
use tower::ServiceExt;
#[path = "console_contract_support/mod.rs"]
mod support;
use support::*;

const SETTINGS: &str = "/v1/internal/gateway/console/diagnostics/settings";
const EXPORT: &str = "/v1/internal/gateway/console/diagnostics/export";

struct TempDirectory(std::path::PathBuf);
impl TempDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "gateway-diagnostics-contract-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> &std::path::Path {
        &self.0
    }
}
impl Drop for TempDirectory {
    fn drop(&mut self) {
        assert_eq!(self.0.parent(), Some(std::env::temp_dir().as_path()));
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn fixture(root: &std::path::Path) -> ConsoleStateFixture {
    let mut fixture = ConsoleStateFixture::new(
        document("managed", "model", "provider-credential-secret"),
        false,
    );
    let state = Arc::get_mut(&mut fixture.state).unwrap();
    state.config.console.state_dir = root.join("state");
    let log = root.join("logs/gateway-runtime-fixture.log");
    fs::create_dir_all(log.parent().unwrap()).unwrap();
    fs::write(
        &log,
        "Backend fixture log\ntoken=first-secret token=second-secret\n",
    )
    .unwrap();
    state.config.console.log_source = Some(log);
    fixture
}

fn request(
    method: &str,
    path: &str,
    token: Option<&str>,
    body: serde_json::Value,
) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(token) = token {
        builder = builder.header("x-management-token", token);
    }
    builder.body(Body::from(body.to_string())).unwrap()
}

#[tokio::test]
async fn diagnostics_require_management_auth_and_keep_remote_access_boundary() {
    let temp = TempDirectory::new();
    let fixture = fixture(temp.path());
    let router = build_router(Arc::clone(&fixture.state));
    for (method, path, body) in [
        ("GET", SETTINGS, serde_json::json!(null)),
        (
            "PUT",
            SETTINGS,
            serde_json::json!({"retentionDays":0,"logLimitMb":0}),
        ),
        ("POST", EXPORT, serde_json::json!({"kind":"logs"})),
    ] {
        for token in [None, Some("wrong-token")] {
            let response = router
                .clone()
                .oneshot(request(method, path, token, body.clone()))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }
    }
    let mut remote = request(
        "GET",
        SETTINGS,
        Some(MANAGEMENT_TOKEN),
        serde_json::json!(null),
    );
    remote.extensions_mut().insert(ConnectInfo(
        "192.0.2.1:1234".parse::<std::net::SocketAddr>().unwrap(),
    ));
    assert_eq!(
        router.oneshot(remote).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn settings_persist_on_backend_and_exports_are_sanitized_no_store() {
    let temp = TempDirectory::new();
    let fixture = fixture(temp.path());
    let router = build_router(Arc::clone(&fixture.state));
    for policy in [
        serde_json::json!({"retentionDays":7,"logLimitMb":100}),
        serde_json::json!({"retentionDays":30,"logLimitMb":200}),
    ] {
        let response = router
            .clone()
            .oneshot(request(
                "PUT",
                SETTINGS,
                Some(MANAGEMENT_TOKEN),
                policy.clone(),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            parse_json(response).await["retentionDays"],
            policy["retentionDays"]
        );
    }
    let response = router
        .clone()
        .oneshot(request(
            "GET",
            SETTINGS,
            Some(MANAGEMENT_TOKEN),
            serde_json::json!(null),
        ))
        .await
        .unwrap();
    let snapshot = parse_json(response).await;
    assert_eq!(snapshot["logLimitMb"], 200);
    assert_eq!(
        snapshot["logDirectory"],
        temp.path().join("logs").display().to_string()
    );
    for kind in ["logs", "diagnostics"] {
        let response = router
            .clone()
            .oneshot(request(
                "POST",
                EXPORT,
                Some(MANAGEMENT_TOKEN),
                serde_json::json!({"kind":kind}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["cache-control"], "no-store");
        let download = parse_json(response).await;
        let content = download["content"].as_str().unwrap();
        for secret in [
            "first-secret",
            "second-secret",
            MANAGEMENT_TOKEN,
            "provider-credential-secret",
            "hmac-contract-secret",
        ] {
            assert!(!content.contains(secret));
        }
        if kind == "logs" {
            assert!(content.contains("Backend fixture log"));
        } else {
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(content).unwrap()["format"],
                "gateway-backend-diagnostics-v1"
            );
        }
    }
}

#[tokio::test]
async fn rejects_arbitrary_paths_unknown_fields_and_unsupported_retention() {
    let temp = TempDirectory::new();
    let fixture = fixture(temp.path());
    let router = build_router(Arc::clone(&fixture.state));
    for (method, path, body) in [
        (
            "PUT",
            SETTINGS,
            serde_json::json!({"retentionDays":1,"logLimitMb":0}),
        ),
        (
            "PUT",
            SETTINGS,
            serde_json::json!({"retentionDays":0,"logLimitMb":0,"path":"../credentials"}),
        ),
        (
            "POST",
            EXPORT,
            serde_json::json!({"kind":"logs","path":"/etc/passwd"}),
        ),
        ("POST", EXPORT, serde_json::json!({"kind":"unknown"})),
    ] {
        let response = router
            .clone()
            .oneshot(request(method, path, Some(MANAGEMENT_TOKEN), body))
            .await
            .unwrap();
        assert!(response.status().is_client_error());
    }
}
