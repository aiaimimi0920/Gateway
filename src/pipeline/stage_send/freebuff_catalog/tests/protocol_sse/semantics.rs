//! The original native reason, not a lossy Responses status, is the public audit fact.
use super::*;
#[tokio::test]
async fn freebuff_public_protocol_sse_preserves_length_and_content_filter_in_original_audit() {
    for path in PATHS {
        for (mode, reason) in [
            ("public_sse_length", "length"),
            ("public_sse_filter", "content_filter"),
        ] {
            let peer = peer::Peer::new().await;
            *peer.mode.lock().unwrap() = mode.into();
            let host = Box::pin(Host::new(&peer)).await;
            host.seed_device(&peer).await;
            let task = request(&host, path, input(path));
            public_sse::approve(&host, &task).await;
            let response = task.await.unwrap();
            assert_eq!(response.status(), 200);
            peer.live.eof.notify_one();
            let body = response.text().await.unwrap();
            assert!(body.contains(if path == PATHS[0] {
                "event: response.incomplete"
            } else {
                "event: message_stop"
            }));
            settled(&host, "completed", 9).await;
            let rows = host
                .state
                .local_runtime
                .as_ref()
                .unwrap()
                .list_audits(&crate::db::RequestAuditFilters::default())
                .await
                .unwrap();
            assert_eq!(
                rows[0].route_trace.as_ref().unwrap()["canonicalCompletionSemantics"],
                reason
            );
            assert_eq!(peer.finishes()[0]["status"], "completed");
            assert_eq!(peer.calls("/api/v1/chat/completions"), 1);
            host.close().await;
            peer.server.close().await;
        }
    }
}
