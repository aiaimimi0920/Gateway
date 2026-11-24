//! Extractor/layer characterization: no AppState, sockets, providers or image decoding.
use axum::body::{to_bytes, Body};
use axum::extract::DefaultBodyLimit;
use axum::http::{header, Request, StatusCode};
use axum::routing::post;
use axum::Router;
use bytes::Bytes;
use neuro_gateway::http::{extractors::JsonBody, middleware::body_limit_layer};
use serde_json::Value;
use std::convert::Infallible;
use tower::ServiceExt;

const EXTRACTOR_DEFAULT: usize = 2 * 1024 * 1024;
const CONFIGURED_LIMIT: usize = 4 * 1024 * 1024;

async fn accepted(JsonBody(_body): JsonBody) -> StatusCode {
    StatusCode::NO_CONTENT
}

fn app(limit: usize, disable_extractor_default: bool) -> Router {
    let router = Router::new()
        .route("/extract", post(accepted))
        .layer(body_limit_layer(limit));
    if disable_extractor_default {
        // Test-only causal control; never change the production limit here.
        router.layer(DefaultBodyLimit::disable())
    } else {
        router
    }
}

fn request(size: usize, streamed: bool) -> Request<Body> {
    let mut data = Vec::with_capacity(size);
    data.extend_from_slice(b"{\"padding\":\"");
    data.resize(size - 2, b'x');
    data.extend_from_slice(b"\"}");
    assert_eq!(data.len(), size);
    let bytes = Bytes::from(data);
    let body = if streamed {
        let split = bytes.len() / 2;
        Body::from_stream(futures::stream::iter([
            Ok::<_, Infallible>(bytes.slice(..split)),
            Ok(bytes.slice(split..)),
        ]))
    } else {
        Body::from(bytes)
    };
    let mut builder = Request::post("/extract").header(header::CONTENT_TYPE, "application/json");
    if !streamed {
        builder = builder.header(header::CONTENT_LENGTH, size.to_string());
    }
    builder.body(body).unwrap()
}

#[tokio::test]
async fn json_extractor_default_overrides_a_larger_configured_limit() {
    for streamed in [false, true] {
        for size in [
            EXTRACTOR_DEFAULT - 1,
            EXTRACTOR_DEFAULT,
            EXTRACTOR_DEFAULT + 1,
        ] {
            let response = app(CONFIGURED_LIMIT, false)
                .oneshot(request(size, streamed))
                .await
                .unwrap();
            if size <= EXTRACTOR_DEFAULT {
                assert_eq!(response.status(), StatusCode::NO_CONTENT);
            } else {
                assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
                let body = to_bytes(response.into_body(), 16 * 1024).await.unwrap();
                let error: Value = serde_json::from_slice(&body).unwrap();
                assert_eq!(error["error"]["code"], "request_too_large");
            }
        }
    }
}

#[tokio::test]
async fn test_only_extractor_override_isolates_the_second_limit() {
    for streamed in [false, true] {
        let response = app(CONFIGURED_LIMIT, true)
            .oneshot(request(EXTRACTOR_DEFAULT + 1, streamed))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }
}

#[tokio::test]
async fn configured_layer_still_rejects_larger_bodies_without_extractor_default() {
    let limit = 64 * 1024;
    for streamed in [false, true] {
        for size in [limit, limit + 1] {
            let response = app(limit, true)
                .oneshot(request(size, streamed))
                .await
                .unwrap();
            let expected = if size == limit {
                StatusCode::NO_CONTENT
            } else {
                StatusCode::PAYLOAD_TOO_LARGE
            };
            // Known-length middleware rejection may be plain text, unlike JsonBody.
            assert_eq!(response.status(), expected);
        }
    }
}
