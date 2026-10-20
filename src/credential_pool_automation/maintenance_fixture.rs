//! Isolated Local runtime plus a mutable trusted driver and verified WebDAV fixture.
use super::*;
use crate::state::AppState;
use axum::{
    body::Bytes,
    http::{Method, StatusCode},
    response::IntoResponse,
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
use tokio::sync::Notify;

#[derive(Default)]
pub(super) struct DriverState {
    pub response: Mutex<Value>,
    pub calls: AtomicUsize,
    pub pause: AtomicBool,
    pub entered: Notify,
    pub release: Notify,
    pub archives: Mutex<HashMap<String, Bytes>>,
    pub pause_archive: AtomicBool,
    pub readback: Notify,
    pub release_archive: Notify,
}

pub(super) struct Fixture {
    pub root: PathBuf,
    pub console: ConsoleConfig,
    pub state: Arc<AppState>,
    pub router: Router,
    pub driver: Arc<DriverState>,
    server: tokio::task::JoinHandle<()>,
}

impl Fixture {
    pub async fn start(remote: bool, permanent_delete: bool) -> Self {
        let root =
            std::env::temp_dir().join(format!("gateway-local-maintenance-{}", Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let driver = Arc::new(DriverState::default());
        *driver.response.lock().unwrap() = json!({"prune":[
            {"credential_id":"a","classification":"permanent_auth_failure"},
            {"credential_id":"b","classification":"account_deleted"}]});
        let handler = Arc::clone(&driver);
        let driver_route = post(move || {
            let handler = Arc::clone(&handler);
            async move {
                handler.calls.fetch_add(1, Ordering::SeqCst);
                if handler.pause.load(Ordering::SeqCst) {
                    handler.entered.notify_one();
                    handler.release.notified().await;
                }
                Json(handler.response.lock().unwrap().clone())
            }
        });
        let storage = Arc::clone(&driver);
        let app = Router::new().route("/driver", driver_route).fallback(
            move |method: Method, uri: axum::http::Uri, body: Bytes| {
                let storage = Arc::clone(&storage);
                async move {
                    let key = uri.path().to_string();
                    match method {
                        Method::GET => {
                            let bytes = storage.archives.lock().unwrap().get(&key).cloned();
                            if bytes.is_some() && storage.pause_archive.load(Ordering::SeqCst) {
                                storage.readback.notify_one();
                                storage.release_archive.notified().await;
                            }
                            match bytes {
                                Some(bytes) => (StatusCode::OK, bytes).into_response(),
                                None => StatusCode::NOT_FOUND.into_response(),
                            }
                        }
                        Method::PUT => {
                            let record: Value = serde_json::from_slice(&body).unwrap();
                            if record["credentialId"] == "b" {
                                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                            }
                            storage.archives.lock().unwrap().insert(key, body);
                            StatusCode::CREATED.into_response()
                        }
                        _ if method.as_str() == "MKCOL" => StatusCode::CREATED.into_response(),
                        _ => StatusCode::METHOD_NOT_ALLOWED.into_response(),
                    }
                }
            },
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let mut provider = json!({"id":"p", "base_url":"https://example.invalid",
            "credential_archive_path":root.join("archive"),
            "credential_permanent_delete_enabled":permanent_delete,
            "credentials":[{"id":"a","api_key":"fixture-private-a"},
                {"id":"b","api_key":"fixture-private-b"}]});
        if remote {
            provider["credential_archive_connection"] = json!({"type":"webdav", "endpoint":format!("{endpoint}/dav"),
                "directory":"pool", "allow_insecure_http":true});
        }
        let routes = root.join("routes.yaml");
        fs::write(
            &routes,
            serde_json::to_vec(&json!({"providers":[provider],"model_routes":[]})).unwrap(),
        )
        .unwrap();
        let drivers = root.join("drivers.json");
        fs::write(
            &drivers,
            serde_json::to_vec(&json!({"drivers":[{"id":"fixture", "provider_ids":["p"],
            "timeout_secs":5, "mode":"http", "endpoint":format!("{endpoint}/driver")}]}))
            .unwrap(),
        )
        .unwrap();
        let console = ConsoleConfig::from_values(ConsoleConfigValues {
            state_dir: Some(root.join("state")),
            routes_file: Some(routes),
            ..Default::default()
        })
        .unwrap();
        let mut config = local_config(console.clone());
        config.credential_pool_automation.enabled = true;
        config.credential_pool_automation.driver_config_path = Some(drivers);
        let state = crate::runtime::build_app_state(config).await.unwrap();
        let router = crate::http::router::build_router(state.clone());
        Self {
            root,
            console,
            state,
            router,
            driver,
            server,
        }
    }

    pub async fn request(&self, suffix: &str, body: Value) -> axum::response::Response {
        self.router
            .clone()
            .oneshot(request(
                suffix,
                body.to_string(),
                Some("fixture-management-token"),
            ))
            .await
            .unwrap()
    }

    pub async fn preview(&self) -> Value {
        let response = self.request("preview", json!({})).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["cache-control"], "no-store");
        decode(response).await["preview"].clone()
    }

    pub async fn close(self) -> crate::routing::config::RouteConfigYaml {
        self.server.abort();
        let _ = self.server.await;
        drop(self.router);
        self.state.local_runtime.as_ref().unwrap().close().await;
        drop(self.state);
        let runtime = RouteConfigRuntime::new_local(&self.console).unwrap();
        let document = runtime.route_config().snapshot().document().clone();
        drop(runtime);
        crate::local_runtime::test_support::remove_test_root(&self.root).await;
        document
    }
}

pub(super) fn request(suffix: &str, body: String, token: Option<&str>) -> Request<Body> {
    let mut request = Request::post(format!(
        "/v1/internal/gateway/credential-pool-automation/providers/p/prune/{suffix}"
    ))
    .header("content-type", "application/json");
    if let Some(token) = token {
        request = request.header("x-management-token", token);
    }
    request.body(Body::from(body)).unwrap()
}

pub(super) fn selection(preview: &Value, ids: Value) -> Value {
    json!({"expectedRevision":preview["revision"], "previewDigest":preview["digest"], "credentialIds":ids})
}

pub(super) async fn decode(response: axum::response::Response) -> Value {
    serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), 65536)
            .await
            .unwrap(),
    )
    .unwrap()
}
