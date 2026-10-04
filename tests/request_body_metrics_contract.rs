//! Real Axum middleware/body path, with only loopback-free fixtures and lazy stores.
#[path = "pipeline_send_runtime/config.rs"]
mod config;
#[path = "pipeline_send_runtime/fixture.rs"]
mod fixture;
mod support;

use axum::{
    body::{Body, Bytes},
    http::{HeaderMap, Request, Response},
    middleware,
    routing::get,
    Router,
};
use http_body::Frame;
use http_body_util::{BodyExt, StreamBody};
use neuro_gateway::{http::middleware::request_logging, metrics::request::global_gateway_metrics};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tower::ServiceExt;

struct ReleaseProbe {
    state: Arc<neuro_gateway::state::AppState>,
    active_at_drop: Arc<AtomicUsize>,
    end_stream: bool,
}
impl http_body::Body for ReleaseProbe {
    type Data = Bytes;
    type Error = std::io::Error;
    fn poll_frame(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
        panic!("HEAD / initial end-stream source must not be polled")
    }
    fn is_end_stream(&self) -> bool {
        self.end_stream
    }
}
impl Drop for ReleaseProbe {
    fn drop(&mut self) {
        self.active_at_drop
            .store(self.state.lifecycle.active_requests(), Ordering::SeqCst);
    }
}

#[tokio::test]
async fn middleware_follows_body_eof_error_drop_and_handler_cancellation_once() {
    let fixture = fixture::TestState::new();
    let state = Arc::clone(&fixture.state);
    let metrics = Arc::clone(global_gateway_metrics());
    let calls = Arc::new(AtomicUsize::new(0));
    let handler_calls = Arc::clone(&calls);
    let head_release = Arc::new(AtomicUsize::new(usize::MAX));
    let empty_release = Arc::new(AtomicUsize::new(usize::MAX));
    let head_probe = Arc::clone(&head_release);
    let empty_probe = Arc::clone(&empty_release);
    let head_state = Arc::clone(&state);
    let empty_state = Arc::clone(&state);
    let app = Router::new()
        .route(
            "/head-probe",
            get(move || {
                let source = ReleaseProbe {
                    state: Arc::clone(&head_state),
                    active_at_drop: Arc::clone(&head_probe),
                    end_stream: false,
                };
                async move { Response::new(Body::new(source)) }
            }),
        )
        .route(
            "/empty-probe",
            get(move || {
                let source = ReleaseProbe {
                    state: Arc::clone(&empty_state),
                    active_at_drop: Arc::clone(&empty_probe),
                    end_stream: true,
                };
                async move { Response::new(Body::new(source)) }
            }),
        )
        .route(
            "/pending",
            get(|| async { std::future::pending::<Response<Body>>().await }),
        )
        .route(
            "/chunked",
            get(|| async {
                let stream = futures::stream::once(async {
                    tokio::time::sleep(Duration::from_millis(30)).await;
                    Ok::<_, std::io::Error>(Bytes::from_static(b"first"))
                })
                .chain(futures::stream::pending());
                Response::new(Body::from_stream(stream))
            }),
        )
        .route(
            "/error",
            get(|| async {
                Response::new(Body::from_stream(futures::stream::iter([Err::<Bytes, _>(
                    std::io::Error::other("synthetic body error"),
                )])))
            }),
        )
        .route(
            "/trailers",
            get(|| async {
                let mut trailers = HeaderMap::new();
                trailers.insert("x-fixture-trailer", "retained".parse().unwrap());
                Response::new(Body::new(StreamBody::new(futures::stream::iter([
                    Ok::<_, std::io::Error>(Frame::data(Bytes::from_static(b"payload"))),
                    Ok(Frame::trailers(trailers)),
                ]))))
            }),
        )
        .route("/empty", get(|| async { Body::empty() }))
        .route("/buffered", get(|| async { "buffered" }))
        .route(
            "/retry",
            get(move || {
                let calls = Arc::clone(&handler_calls);
                async move {
                    use neuro_gateway::{
                        error::GatewayError,
                        retry::{execute_with_retry_after_admission_observed, RetryPolicy},
                    };
                    let policy = RetryPolicy {
                        initial_delay: Duration::ZERO,
                        ..Default::default()
                    };
                    execute_with_retry_after_admission_observed(
                        || {
                            let attempt = calls.fetch_add(1, Ordering::SeqCst);
                            async move {
                                if attempt == 0 {
                                    Err(GatewayError::server_error("synthetic unavailable"))
                                } else {
                                    Ok("recovered")
                                }
                            }
                        },
                        || async { Ok(()) },
                        |observation| {
                            global_gateway_metrics().observe_provider_outcome(
                                neuro_gateway::metrics::request::ProviderMetricOutcome {
                                    provider: "fixture",
                                    model: Some("fixture-model"),
                                    success: observation.succeeded,
                                    latency_ms: observation.latency_ms,
                                    failure_class: Some("provider_transient"),
                                },
                            );
                        },
                        &policy,
                    )
                    .await
                    .unwrap()
                }
            }),
        )
        .layer(middleware::from_fn_with_state(
            Arc::clone(&state),
            request_logging,
        ));

    use futures::StreamExt;
    let baseline = metrics.snapshot().requests_total;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/chunked")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        metrics.snapshot().requests_total,
        baseline,
        "response headers are not terminal"
    );
    assert_eq!(state.lifecycle.active_requests(), 1);
    let mut body = response.into_body();
    assert_eq!(
        body.frame().await.unwrap().unwrap().into_data().unwrap(),
        "first"
    );
    drop(body);
    assert_eq!(metrics.snapshot().requests_total, baseline + 1);
    assert_eq!(state.lifecycle.active_requests(), 0);
    assert!(metrics.snapshot().request_duration_ms_sum >= 30);
    assert!(metrics
        .render_prometheus()
        .contains("gateway_request_terminations_total{reason=\"cancelled\"} 1"));

    let mut pending = Box::pin(
        app.clone().oneshot(
            Request::builder()
                .uri("/pending")
                .body(Body::empty())
                .unwrap(),
        ),
    );
    assert!(futures::poll!(pending.as_mut()).is_pending());
    assert_eq!(state.lifecycle.active_requests(), 1);
    drop(pending);
    assert_eq!(state.lifecycle.active_requests(), 0);
    assert_eq!(metrics.snapshot().requests_total, baseline + 2);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/error")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(response.into_body().collect().await.is_err());
    assert_eq!(metrics.snapshot().requests_total, baseline + 3);
    assert!(metrics
        .render_prometheus()
        .contains("gateway_request_terminations_total{reason=\"stream_interrupted\"} 1"));

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/trailers")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let collected = response.into_body().collect().await.unwrap();
    assert_eq!(
        collected.trailers().unwrap()["x-fixture-trailer"],
        "retained"
    );
    assert_eq!(collected.to_bytes(), "payload");
    assert_eq!(metrics.snapshot().requests_total, baseline + 4);

    for (index, path) in ["/empty", "/buffered", "/retry"].iter().enumerate() {
        let response = app
            .clone()
            .oneshot(Request::builder().uri(*path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        response.into_body().collect().await.unwrap();
        assert_eq!(
            metrics.snapshot().requests_total,
            baseline + 5 + index as u64
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert!(metrics.render_prometheus().contains(
        "gateway_provider_requests_total{provider=\"fixture\",model=\"fixture-model\"} 2"
    ));
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("HEAD")
                .uri("/buffered")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    drop(response);
    assert_eq!(metrics.snapshot().requests_total, baseline + 8);
    for (index, (method, path, released)) in [
        ("HEAD", "/head-probe", head_release),
        ("GET", "/empty-probe", empty_release),
    ]
    .into_iter()
    .enumerate()
    {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            released.load(Ordering::SeqCst),
            1,
            "release source before decrementing in-flight"
        );
        drop(response);
        assert_eq!(state.lifecycle.active_requests(), 0);
        assert_eq!(
            metrics.snapshot().requests_total,
            baseline + 9 + index as u64
        );
    }
    state.lifecycle.begin_drain("fixture");
    let response = app
        .oneshot(
            Request::builder()
                .uri("/buffered")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 503);
    response.into_body().collect().await.unwrap();
    assert_eq!(metrics.snapshot().requests_total, baseline + 11);
    assert_eq!(metrics.snapshot().request_drain_rejections_total, 1);
    assert_eq!(metrics.snapshot().request_in_flight, 0);
    assert_eq!(state.lifecycle.active_requests(), 0);
    drop(state);
    fixture.finish().await;
}
