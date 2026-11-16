//! Real stage_send wiring with synthetic AWS bytes and random loopback ports only.
#![cfg(feature = "family-bedrock-converse-official-api")]
#[path = "pipeline_send_runtime/config.rs"]
mod config;
#[path = "pipeline_send_runtime/fixture.rs"]
mod fixture;
mod support;
#[path = "bedrock_upstream_eventstream_contract/fixtures.rs"]
mod wire;

use axum::{routing::post, Router};
use futures::StreamExt;
use neuro_gateway::{
    pipeline::{stage_send, PipelineOutput},
    routing::candidate::{ProviderExecutionMode, RouteCandidate},
};
use serde_json::json;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{sync::oneshot, task::JoinHandle, time::timeout};

struct Server {
    base: String,
    calls: Arc<AtomicUsize>,
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}
impl Server {
    async fn new(body: Vec<u8>) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        let router = Router::new().route(
            "/model/upstream-model/converse-stream",
            post(move || {
                let body = body.clone();
                let observed = observed.clone();
                async move {
                    observed.fetch_add(1, Ordering::SeqCst);
                    (
                        [("content-type", "application/vnd.amazon.eventstream")],
                        body,
                    )
                }
            }),
        );
        let (stop, stopped) = oneshot::channel();
        let task = tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
                })
                .await
                .unwrap();
        });
        Self {
            base,
            calls,
            stop: Some(stop),
            task,
        }
    }
    async fn finish(mut self) {
        let _ = self.stop.take().unwrap().send(());
        timeout(Duration::from_secs(2), &mut self.task)
            .await
            .unwrap()
            .unwrap();
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn candidate(server: &Server) -> RouteCandidate {
    RouteCandidate {
        provider_account_id: format!("cred:bedrock-fixture-{}", uuid::Uuid::new_v4()),
        provider_credential_id: None,
        label: "bedrock-wire".into(),
        payload: serde_json::from_value(
            json!({"adapter":"bedrock_converse_compatible","base_url":server.base,
            "api_key":"synthetic-only","execution_mode":"direct_http"}),
        )
        .unwrap(),
        protocol_family: "bedrock_converse".into(),
        protocol_profile: "aws_bedrock".into(),
        supported_protocol_families: vec!["bedrock_converse".into()],
        adapter: "bedrock_converse_compatible".into(),
        model_alias: None,
        upstream_model: Some("upstream-model".into()),
        resolved_execution_mode: ProviderExecutionMode::DirectHttp,
        priority: 100,
        weight: 1,
        failure_count: 0,
        cooldown_until: None,
        routing_score: None,
        routing_health_weight: None,
        routing_capacity_weight: None,
        routing_degraded: None,
        routing_breaker_open: None,
        routing_degradation_reasons: vec![],
    }
}

#[tokio::test]
async fn bedrock_pipeline_stream_normalizes_real_aws_frames_and_finalizes_once() {
    let server = Server::new(wire::sequence().concat()).await;
    let fixture = fixture::TestState::new();
    let selected = candidate(&server);
    let controller = fixture
        .state
        .concurrency_registry
        .get_or_create(&selected.provider_account_id);
    let mut ctx = fixture::context(true, vec![selected]);
    let output = timeout(fixture::DEADLINE, stage_send::run(&mut ctx, &fixture.state))
        .await
        .unwrap()
        .unwrap();
    let PipelineOutput::Sse(mut stream) = output else {
        panic!("expected SSE")
    };
    assert_eq!(controller.snapshot().consecutive_successes, 0);
    let bytes = timeout(fixture::DEADLINE, async {
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.next().await {
            bytes.extend_from_slice(&chunk.unwrap());
            assert!(bytes.len() <= 65536);
        }
        bytes
    })
    .await
    .unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.contains("你好"));
    assert!(text.contains("call-1"));
    assert!(text.contains("\"total_tokens\":13"));
    assert!(text.contains("\"index\":0"));
    assert_eq!(text.matches("[DONE]").count(), 1);
    assert_eq!(controller.snapshot().consecutive_successes, 1);
    drop(stream);
    assert_eq!(controller.snapshot().consecutive_successes, 1);
    assert_eq!(controller.snapshot().active_count, 0);
    assert_eq!(ctx.route_attempt_count.load(Ordering::Relaxed), 1);
    assert_eq!(server.calls.load(Ordering::SeqCst), 1);
    server.finish().await;
    fixture.finish().await;
}

#[tokio::test]
async fn bedrock_pipeline_buffered_uses_the_native_decoder() {
    let server = Server::new(wire::sequence().concat()).await;
    let fixture = fixture::TestState::new();
    let mut ctx = fixture::context(false, vec![candidate(&server)]);
    let output = timeout(fixture::DEADLINE, stage_send::run(&mut ctx, &fixture.state))
        .await
        .unwrap()
        .unwrap();
    let PipelineOutput::Json(body) = output else {
        panic!("expected JSON")
    };
    assert_eq!(body["choices"][0]["message"]["content"], "你好");
    assert_eq!(
        body["choices"][0]["message"]["tool_calls"][0]["id"],
        "call-1"
    );
    assert_eq!(body["usage"]["total_tokens"], 13);
    assert_eq!(server.calls.load(Ordering::SeqCst), 1);
    server.finish().await;
    fixture.finish().await;
}

#[tokio::test]
async fn bedrock_pipeline_partial_failure_and_consumer_drop_do_not_count_as_success() {
    for fail in [true, false] {
        let mut frames = wire::sequence();
        if fail {
            frames.pop();
        }
        let server = Server::new(frames.concat()).await;
        let fixture = fixture::TestState::new();
        let selected = candidate(&server);
        let controller = fixture
            .state
            .concurrency_registry
            .get_or_create(&selected.provider_account_id);
        let mut ctx = fixture::context(true, vec![selected]);
        let output = timeout(fixture::DEADLINE, stage_send::run(&mut ctx, &fixture.state))
            .await
            .unwrap()
            .unwrap();
        let PipelineOutput::Sse(mut stream) = output else {
            panic!("expected SSE")
        };
        if fail {
            let mut errors = 0;
            timeout(fixture::DEADLINE, async {
                while let Some(item) = stream.next().await {
                    match item {
                        Err(_) => errors += 1,
                        Ok(bytes) => assert!(!String::from_utf8_lossy(&bytes).contains("[DONE]")),
                    }
                }
            })
            .await
            .unwrap();
            assert_eq!(errors, 1);
        }
        drop(stream);
        assert_eq!(controller.snapshot().consecutive_successes, 0);
        assert_eq!(controller.snapshot().active_count, 0);
        assert_eq!(server.calls.load(Ordering::SeqCst), 1);
        server.finish().await;
        fixture.finish().await;
    }
}
