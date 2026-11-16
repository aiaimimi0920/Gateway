//! The handed-off original owner distinguishes cooperative cancellation, real Drop and terminal Drop.
use super::*;
use crate::pipeline::{request_budget::RequestBudget, PipelineContext, PipelineOutput};
#[tokio::test]
async fn freebuff_public_chat_sse_handoff_cooperates_with_caller_closed_shutdown_and_deadline() {
    for cause in [
        "caller",
        "closed",
        "shutdown",
        "deadline",
        "drop",
        "done_drop",
    ] {
        let peer = peer::Peer::new().await;
        *peer.mode.lock().unwrap() = "public_sse_known".into();
        let host = Box::pin(Host::new(&peer)).await;
        host.seed_device(&peer).await;
        let mut body = request();
        body["stream"] = json!(true);
        let req = crate::protocol::openai::normalize_chat_completions(body).unwrap();
        let mut ctx = PipelineContext::new(req, Some(host.token.clone()));
        ctx.request_budget = RequestBudget::new(
            1,
            Duration::from_secs(if cause == "deadline" { 2 } else { 10 }),
        );
        let budget = ctx.request_budget.clone();
        let (cancel, receiver) = tokio::sync::watch::channel(false);
        let state = host.state.clone();
        let origin = peer.server.origin.clone();
        let task = tokio::spawn(async move {
            crate::protocol::freebuff::catalog::with_test_origin(
                origin,
                crate::pipeline::run_pipeline_cancellable(ctx, &state, receiver),
            )
            .await
        });
        approve(&host, &task).await;
        let mut stream = match task.await.unwrap().unwrap() {
            PipelineOutput::Sse(stream) => stream,
            _ => panic!("expected public SSE"),
        };
        let first = stream.next().await.unwrap().unwrap();
        assert!(std::str::from_utf8(&first).unwrap().contains("partial"));
        assert!(
            budget.check_inflight().is_err(),
            "handoff still fences all ordinary/new sends"
        );
        assert_eq!(
            host.state
                .concurrency_registry
                .get_or_create("pool")
                .snapshot()
                .active_count,
            1
        );
        if cause == "drop" {
            drop(stream);
            drop(cancel);
            settled(&host, "cancelled", 10).await;
            assert!(peer.finishes().is_empty());
        } else if cause == "done_drop" {
            peer.live.eof.notify_one();
            loop {
                let bytes = stream.next().await.unwrap().unwrap();
                if std::str::from_utf8(&bytes).unwrap().contains("[DONE]") {
                    break;
                }
            }
            drop(stream);
            drop(cancel);
            settled(&host, "completed", 9).await;
            assert_eq!(peer.finishes().len(), 1);
            assert_eq!(peer.finishes()[0]["status"], "completed");
        } else {
            let keep = match cause {
                "caller" => {
                    cancel.send(true).unwrap();
                    Some(cancel)
                }
                "closed" => {
                    drop(cancel);
                    None
                }
                "shutdown" => {
                    host.state.shutdown.request("public_sse_shutdown");
                    Some(cancel)
                }
                "deadline" => Some(cancel),
                _ => unreachable!(),
            };
            let mut bytes = Vec::new();
            let error = tokio::time::timeout(Duration::from_secs(4), async {
                loop {
                    match stream.next().await {
                        Some(Ok(chunk)) => bytes.extend_from_slice(&chunk),
                        Some(Err(error)) => break error,
                        None => panic!("cancelled stream cannot reach successful EOF"),
                    }
                }
            })
            .await
            .unwrap();
            assert!(!String::from_utf8(bytes).unwrap().contains("[DONE]"));
            let error = match error {
                crate::protocol::stream_error::StreamError::Protocol(error) => {
                    error.gateway_error().unwrap().clone()
                }
                _ => panic!("expected safe classified owner failure"),
            };
            assert_eq!(
                error.request_budget_stop_reason(),
                Some(match cause {
                    "caller" | "closed" => "caller_cancelled",
                    other => other,
                })
            );
            drop(stream);
            drop(keep);
            assert_eq!(peer.finishes().len(), 1);
            assert_eq!(peer.finishes()[0]["status"], "cancelled");
            assert_eq!(peer.finishes()[0]["totalCredits"], 13);
            settled(
                &host,
                if cause == "deadline" {
                    "failed"
                } else {
                    "cancelled"
                },
                10,
            )
            .await;
            let control = host
                .state
                .concurrency_registry
                .get_or_create("pool")
                .snapshot();
            assert_eq!(control.consecutive_successes, 0);
        }
        assert_eq!(peer.calls("/api/v1/chat/completions"), 1);
        assert_eq!(peer.calls("/api/v1/freebuff/session/admission"), 1);
        assert_eq!(peer.calls("/api/v1/freebuff/session/attempt"), 0);
        assert_eq!(claims(&host).await, 1);
        tokio::time::timeout(Duration::from_secs(3), async {
            while !peer.live.dropped.load(Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        host.close().await;
        peer.server.close().await;
    }
}
