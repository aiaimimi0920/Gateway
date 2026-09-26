use super::{
    ManagedWorker, SplitterManager, WorkerRequestLease, SPLITTER_RELOAD_PATH, SPLITTER_STATUS_PATH,
};
use anyhow::anyhow;
use axum::body::{to_bytes, Body};
use axum::extract::{Request, State};
use axum::http::header::{
    CONNECTION, HOST, PROXY_AUTHENTICATE, PROXY_AUTHORIZATION, TE, TRAILER, TRANSFER_ENCODING,
    UPGRADE,
};
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures::StreamExt;
pub(super) async fn ensure_worker_available(
    State(manager): State<SplitterManager>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();
    if path == "/healthz"
        || path == "/readyz"
        || path == SPLITTER_STATUS_PATH
        || path == SPLITTER_RELOAD_PATH
    {
        return next.run(request).await;
    }

    if manager.routing_worker().is_none() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": {
                    "message": "Gateway splitter has no available worker",
                    "code": "gateway_splitter_unavailable",
                }
            })),
        )
            .into_response();
    }

    next.run(request).await
}

pub(super) async fn proxy_request(
    State(manager): State<SplitterManager>,
    request: Request<Body>,
) -> impl IntoResponse {
    manager.reconcile_worker_processes().await;
    let Some(worker) = manager.routing_worker() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": {
                    "message": "Gateway splitter has no available worker",
                    "code": "gateway_splitter_unavailable",
                }
            })),
        )
            .into_response();
    };
    let Some(lease) = worker.try_acquire_lease() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": {
                    "message": "Gateway splitter worker entered drain before request admission",
                    "code": "gateway_splitter_unavailable",
                }
            })),
        )
            .into_response();
    };

    let (parts, body) = request.into_parts();
    let body_bytes = match to_bytes(body, manager.max_request_body_bytes()).await {
        Ok(bytes) => bytes,
        Err(error) => {
            return (
                StatusCode::PAYLOAD_TOO_LARGE,
                Json(serde_json::json!({
                    "error": {
                        "message": format!("Request body too large: {}", error),
                        "code": "request_too_large",
                    }
                })),
            )
                .into_response();
        }
    };

    match send_request_to_worker(&manager, worker.as_ref(), &parts, body_bytes, lease).await {
        Ok(response) => response,
        Err(error) => {
            tracing::warn!(
                worker_id = %worker.id,
                ?error,
                "Failed to proxy request to active worker"
            );
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "error": {
                        "message": error.to_string(),
                        "code": "gateway_splitter_proxy_failed",
                    }
                })),
            )
                .into_response()
        }
    }
}

async fn send_request_to_worker(
    manager: &SplitterManager,
    worker: &ManagedWorker,
    parts: &axum::http::request::Parts,
    body_bytes: bytes::Bytes,
    lease: WorkerRequestLease,
) -> anyhow::Result<Response<Body>> {
    let target = format!(
        "{}{}",
        worker.base_url,
        parts
            .uri
            .path_and_query()
            .map(|value| value.as_str())
            .unwrap_or("/")
    );

    let mut builder = manager
        .inner
        .proxy_client
        .request(parts.method.clone(), target);
    for (name, value) in &parts.headers {
        if !is_hop_by_hop_header(name) {
            builder = builder.header(name, value.clone());
        }
    }
    builder = builder.header("x-gateway-splitter-worker-id", worker.id.as_str());
    builder = builder.body(body_bytes);

    let upstream = builder.send().await?;
    let status = upstream.status();
    let upstream_headers = upstream.headers().clone();
    let stream = Box::pin(upstream.bytes_stream());
    let stream = futures::stream::unfold((stream, lease), |(mut stream, lease)| async move {
        stream.next().await.map(|item| {
            (
                item.map_err(|error| std::io::Error::other(error.to_string())),
                (stream, lease),
            )
        })
    });

    let mut response = Response::builder()
        .status(status)
        .body(Body::from_stream(stream))
        .map_err(|error| anyhow!("failed to build worker response: {}", error))?;

    for (name, value) in &upstream_headers {
        if !is_hop_by_hop_header(name) {
            response.headers_mut().append(name, value.clone());
        }
    }
    response.headers_mut().insert(
        HeaderName::from_static("x-gateway-splitter-worker-id"),
        HeaderValue::from_str(worker.id.as_str())
            .unwrap_or_else(|_| HeaderValue::from_static("unknown")),
    );

    Ok(response)
}

pub(super) fn is_hop_by_hop_header(name: &HeaderName) -> bool {
    matches!(
        *name,
        CONNECTION
            | HOST
            | PROXY_AUTHENTICATE
            | PROXY_AUTHORIZATION
            | TE
            | TRAILER
            | TRANSFER_ENCODING
            | UPGRADE
    )
}
