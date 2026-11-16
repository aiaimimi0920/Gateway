//! Handed-off protocol variants retain the same cancellation, deadline, permit and original FINISH.
use super::*;
use crate::pipeline::{request_budget::RequestBudget, PipelineContext, PipelineOutput};
#[tokio::test]
async fn freebuff_public_protocol_sse_caller_closed_shutdown_deadline_drop_and_terminal_drop() {
    for path in PATHS {
        for cause in [
            "caller",
            "closed",
            "shutdown",
            "deadline",
            "drop",
            "terminal_drop",
        ] {
            let peer = peer::Peer::new().await;
            *peer.mode.lock().unwrap() = "public_sse_known".into();
            let host = Box::pin(Host::new(&peer)).await;
            host.seed_device(&peer).await;
            let req = if path == PATHS[0] {
                crate::protocol::responses::normalize_responses(input(path)).unwrap()
            } else {
                crate::protocol::anthropic::normalize_messages(input(path)).unwrap()
            };
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
            public_sse::approve(&host, &task).await;
            let mut stream = match task.await.unwrap().unwrap() {
                PipelineOutput::Sse(stream) => stream,
                _ => panic!("expected SSE"),
            };
            assert!(std::str::from_utf8(&stream.next().await.unwrap().unwrap())
                .unwrap()
                .contains("partial"));
            assert!(budget.check_inflight().is_err());
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
                assert!(peer.finishes().is_empty());
                settled(&host, "cancelled", 10).await;
            } else if cause == "terminal_drop" {
                peer.live.eof.notify_one();
                while !std::str::from_utf8(&stream.next().await.unwrap().unwrap())
                    .unwrap()
                    .contains(terminal(path))
                {}
                drop(stream);
                drop(cancel);
                assert_eq!(peer.finishes().len(), 1);
                settled(&host, "completed", 9).await;
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
                        host.state.shutdown.request("protocol_sse_shutdown");
                        Some(cancel)
                    }
                    "deadline" => Some(cancel),
                    _ => unreachable!(),
                };
                let error = tokio::time::timeout(Duration::from_secs(4), async {
                    loop {
                        match stream.next().await {
                            Some(Ok(bytes)) => assert!(!std::str::from_utf8(&bytes)
                                .unwrap()
                                .contains(terminal(path))),
                            Some(Err(error)) => break error,
                            None => panic!("cancelled owner cannot reach successful EOF"),
                        }
                    }
                })
                .await
                .unwrap();
                let error = match error {
                    crate::protocol::stream_error::StreamError::Protocol(error) => {
                        error.gateway_error().unwrap().clone()
                    }
                    _ => panic!("classified owner error required"),
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
            }
            assert_eq!(peer.calls("/api/v1/chat/completions"), 1);
            assert_eq!(peer.calls("/api/v1/freebuff/session/admission"), 1);
            assert_eq!(peer.calls("/api/v1/freebuff/session/attempt"), 0);
            assert_eq!(claims(&host).await, 1);
            host.close().await;
            peer.server.close().await;
        }
    }
}
