use std::convert::Infallible;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use aws_sdk_s3::config::{timeout::TimeoutConfig, Credentials, Region};
use axum::body::Bytes;
use axum::body::{to_bytes, Body};
use axum::extract::{Request, State};
use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Router;
use futures::stream;
use tokio::task::JoinHandle;
use tokio::time::sleep;

use super::super::{
    build_gateway_provider_account_object_key, GatewayObjectStorage, ObjectStorageDriver,
};

#[derive(Default)]
struct ObjectState {
    body: Mutex<Vec<u8>>,
    requests: Mutex<Vec<(Method, String)>>,
}

struct Fixture {
    storage: GatewayObjectStorage,
    state: Arc<ObjectState>,
    server: JoinHandle<()>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}

async fn fixture() -> Fixture {
    let state = Arc::new(ObjectState::default());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind S3 object fixture");
    let address = listener.local_addr().expect("S3 object fixture address");
    let router = Router::new()
        .fallback(serve_object)
        .with_state(state.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, router)
            .await
            .expect("serve S3 object fixture");
    });
    let config = aws_sdk_s3::config::Builder::new()
        .region(Region::new("us-east-1"))
        .credentials_provider(Credentials::new(
            "fixture-access",
            "fixture-secret",
            None,
            None,
            "fixture",
        ))
        .endpoint_url(format!("http://{address}"))
        .force_path_style(true)
        .behavior_version_latest()
        .timeout_config(
            TimeoutConfig::builder()
                .connect_timeout(Duration::from_secs(1))
                .read_timeout(Duration::from_secs(1))
                .operation_attempt_timeout(Duration::from_secs(1))
                .operation_timeout(Duration::from_secs(1))
                .build(),
        )
        .build();
    Fixture {
        storage: GatewayObjectStorage {
            driver: ObjectStorageDriver::S3Compatible {
                client: aws_sdk_s3::Client::from_conf(config),
                bucket: "fixture-bucket".to_string(),
                deadline: Duration::from_millis(100),
            },
        },
        state,
        server,
    }
}

async fn serve_object(State(state): State<Arc<ObjectState>>, request: Request) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let query = request.uri().query().unwrap_or_default();
    let is_slow = path.ends_with("/slow") || query.contains("slow");
    let is_slow_body = path.ends_with("/slow-body");
    state
        .requests
        .lock()
        .expect("object request ledger")
        .push((method.clone(), path));

    if is_slow {
        sleep(Duration::from_secs(1)).await;
    }

    match method {
        Method::PUT => {
            let body = to_bytes(request.into_body(), 8 * 1024 * 1024)
                .await
                .expect("read S3 PUT body");
            *state.body.lock().expect("object body") = body.to_vec();
            StatusCode::OK.into_response()
        }
        Method::GET => {
            let body = state.body.lock().expect("object body").clone();
            if is_slow_body {
                return Response::builder()
                    .status(StatusCode::OK)
                    .header("content-type", "application/json")
                    .header("content-length", body.len())
                    .body(Body::from_stream(stream::once(async move {
                        sleep(Duration::from_secs(1)).await;
                        Ok::<Bytes, Infallible>(Bytes::from(body))
                    })))
                    .expect("build stalled S3 GET response");
            }
            Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/json")
                .header("content-length", body.len())
                .body(Body::from(body))
                .expect("build S3 GET response")
        }
        _ => StatusCode::OK.into_response(),
    }
}

#[tokio::test]
async fn s3_provider_account_object_key_round_trips_json() {
    let fixture = fixture().await;
    let object_key = build_gateway_provider_account_object_key("account-fixture");
    let payload = serde_json::json!({
        "baseUrl": "https://example.invalid",
        "defaultModel": "qwen3-coder-plus"
    });

    fixture
        .storage
        .put_json(&object_key, &payload)
        .await
        .expect("S3 provider account object must be writable");
    assert_eq!(
        fixture.storage.read_json(&object_key).await.unwrap(),
        payload
    );
    fixture
        .storage
        .delete_object(&object_key)
        .await
        .expect("S3 provider account object must be deletable");

    let requests = fixture
        .state
        .requests
        .lock()
        .expect("object request ledger");
    assert!(requests.iter().any(|(method, path)| {
        *method == Method::PUT
            && path.contains("/fixture-bucket/ai-gateway/provider-account/account-fixture.json")
    }));
    assert!(requests.iter().any(|(method, path)| {
        *method == Method::GET
            && path.contains("/fixture-bucket/ai-gateway/provider-account/account-fixture.json")
    }));
    assert!(requests.iter().any(|(method, path)| {
        *method == Method::DELETE
            && path.contains("/fixture-bucket/ai-gateway/provider-account/account-fixture.json")
    }));
}

#[tokio::test]
async fn s3_put_network_deadline_bounds_a_stalled_response() {
    let fixture = fixture().await;
    let error = fixture
        .storage
        .put_bytes("slow", b"payload".to_vec(), "application/octet-stream")
        .await
        .expect_err("stalled S3 response must hit the network deadline");

    assert_eq!(
        error.code.as_deref(),
        Some("object_storage_network_timeout")
    );
}

#[tokio::test]
async fn s3_get_network_deadline_bounds_a_stalled_body() {
    let fixture = fixture().await;
    fixture
        .storage
        .put_bytes("seed", b"payload".to_vec(), "application/octet-stream")
        .await
        .expect("seed S3 body");
    let error = fixture
        .storage
        .read_bytes("slow-body")
        .await
        .expect_err("stalled S3 body must hit the network deadline");

    assert_eq!(
        error.code.as_deref(),
        Some("object_storage_network_timeout")
    );
}

#[tokio::test]
async fn s3_list_network_deadline_bounds_a_stalled_response() {
    let fixture = fixture().await;
    let error = fixture
        .storage
        .list_objects("slow")
        .await
        .expect_err("stalled S3 listing must hit the network deadline");

    assert_eq!(
        error.code.as_deref(),
        Some("object_storage_network_timeout")
    );
}
