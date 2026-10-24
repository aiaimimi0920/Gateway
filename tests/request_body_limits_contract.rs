//! Production router admission, without credentials or upstream model calls.
#[path = "pipeline_send_runtime/config.rs"]
mod config;
#[path = "pipeline_send_runtime/fixture.rs"]
mod fixture;
mod support;

use axum::{body::Body, http::Request};
use http_body_util::BodyExt;
use neuro_gateway::http::router::build_router;
use std::sync::Arc;
use tower::ServiceExt;

const MIB: usize = 1024 * 1024;

#[tokio::test]
async fn configured_body_limits_replace_axum_default_without_removing_bounds() {
    verify_limits(4 * MIB, 3 * MIB).await;
    verify_limits(3 * MIB, 4 * MIB).await;
}

async fn verify_limits(global_limit: usize, responses_limit: usize) {
    let mut fixture = fixture::TestState::new();
    let config = &mut Arc::get_mut(&mut fixture.state).unwrap().config;
    config.max_request_body_bytes = global_limit;
    config.max_body_responses_bytes = responses_limit;
    let app = build_router(Arc::clone(&fixture.state));
    for path in [
        "/v1/responses",
        "/v1/responses/compact",
        "/v1/new-api/responses",
    ] {
        for streamed in [false, true] {
            for (size, expected) in [(2 * MIB + 1, 400), (3 * MIB, 400), (3 * MIB + 1, 413)] {
                // Valid JSON reaches semantic validation, but no model is supplied.
                let mut payload = " ".repeat(size - 2);
                payload.push_str("{}");
                let body = if streamed {
                    Body::from_stream(futures::stream::iter(
                        payload
                            .as_bytes()
                            .chunks(16384)
                            .map(|chunk| {
                                Ok::<_, std::io::Error>(bytes::Bytes::copy_from_slice(chunk))
                            })
                            .collect::<Vec<_>>(),
                    ))
                } else {
                    Body::from(payload)
                };
                let mut request = Request::post(path).header("content-type", "application/json");
                if !streamed {
                    request = request.header("content-length", size);
                }
                let response = app
                    .clone()
                    .oneshot(request.body(body).unwrap())
                    .await
                    .unwrap();
                let status = response.status().as_u16();
                let text = response.into_body().collect().await.unwrap().to_bytes();
                assert_eq!(
                    status, expected,
                    "{path} size={size} streamed={streamed}: {text:?}"
                );
            }
        }
    }
    // A route-local extractor limit must override the global extractor setting.
    for streamed in [false, true] {
        let payload = " ".repeat(65537);
        let body = if streamed {
            Body::from_stream(futures::stream::once(async move {
                Ok::<_, std::io::Error>(bytes::Bytes::from(payload))
            }))
        } else {
            Body::from(payload)
        };
        let mut request = Request::post(
            "/v1/internal/gateway/credential-pool-automation/providers/fixture/prune/apply",
        )
        .header("content-type", "application/json");
        if !streamed {
            request = request.header("content-length", 65537);
        }
        let response = app
            .clone()
            .oneshot(request.body(body).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), 413);
        response.into_body().collect().await.unwrap();
    }
    drop(app);
    fixture.finish().await;
}
