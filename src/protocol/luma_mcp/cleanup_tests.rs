//! Remote cleanup is separate from the catalog outcome and never retried.
use super::{fixture::*, inspection::inspect_at, McpError, VERSION};
use serde_json::json;
use std::time::Duration;

fn replies(session: bool) -> Vec<Reply> {
    let mut init = Reply::json(json!({"jsonrpc":"2.0","id":1,"result":{
        "protocolVersion":VERSION,"capabilities":{"tools":{}}}}));
    init.session = session.then_some("cleanup-session");
    vec![
        init,
        Reply::initialized(),
        Reply::json(json!({
        "jsonrpc":"2.0","id":2,"result":{"tools":[]}})),
    ]
}

async fn inspect(peer: &Peer) -> Result<Vec<super::ToolDefinition>, McpError> {
    inspect_at(
        &http(),
        &peer.endpoint,
        "cleanup-token",
        None,
        Duration::from_secs(1),
    )
    .await
}

#[tokio::test]
async fn luma_mcp_cleanup_closes_session_without_default_credentials() {
    let peer = Peer::new(replies(true)).await;
    assert!(inspect(&peer).await.unwrap().is_empty());
    {
        let deletes = peer.deletes.lock().unwrap();
        assert_eq!(deletes.len(), 1);
        assert_eq!(deletes[0]["authorization"], "Bearer cleanup-token");
        assert_eq!(deletes[0]["mcp-session-id"], "cleanup-session");
        assert_eq!(deletes[0]["mcp-protocol-version"], VERSION);
        assert!(!deletes[0].contains_key("cookie"));
        assert!(!deletes[0].contains_key("x-api-key"));
    }
    peer.stop().await;
}

#[tokio::test]
async fn luma_mcp_cleanup_skips_stateless_and_preserves_catalog_on_rejection() {
    let peer = Peer::new(replies(false)).await;
    assert!(inspect(&peer).await.is_ok());
    assert!(peer.deletes.lock().unwrap().is_empty());
    peer.stop().await;
    for status in [404, 405, 401, 500, 302] {
        let peer = Peer::with_delete(replies(true), status, Duration::ZERO).await;
        assert!(inspect(&peer).await.is_ok());
        assert_eq!(peer.deletes.lock().unwrap().len(), 1);
        peer.stop().await;
    }
}

#[tokio::test]
async fn luma_mcp_cleanup_preserves_original_failure_and_timeout() {
    for timeout in [false, true] {
        let mut responses = replies(true);
        if timeout {
            responses[2].hold = true;
        } else {
            responses[2].body = "invalid".into();
        }
        let peer = Peer::new(responses).await;
        let error = inspect(&peer).await.unwrap_err();
        if timeout {
            // Existing transport may win the shared deadline before the outer timeout.
            assert!(matches!(error, McpError::Timeout | McpError::Transport));
        } else {
            assert_eq!(error, McpError::InvalidResponse);
        }
        assert_eq!(peer.deletes.lock().unwrap().len(), 1);
        peer.stop().await;
    }
}

#[tokio::test]
async fn luma_mcp_cleanup_has_bounded_wait_without_retry() {
    for catalog_timeout in [false, true] {
        let mut responses = replies(true);
        responses[2].hold = catalog_timeout;
        let peer = Peer::with_delete(responses, 204, Duration::from_secs(30)).await;
        let result = tokio::time::timeout(Duration::from_secs(4), inspect(&peer))
            .await
            .unwrap();
        if catalog_timeout {
            assert!(matches!(
                result.unwrap_err(),
                McpError::Timeout | McpError::Transport
            ));
        } else {
            assert!(result.is_ok());
        }
        assert_eq!(peer.deletes.lock().unwrap().len(), 1);
        peer.stop().await;
    }
}

#[tokio::test]
async fn luma_mcp_cleanup_classifies_terminal_status_and_never_replays() {
    use super::{cleanup::Cleanup, transport::Connection};
    for (status, expected) in [
        (204, Cleanup::Closed),
        (200, Cleanup::Closed),
        (404, Cleanup::AlreadyClosed),
        (405, Cleanup::Unsupported),
        (202, Cleanup::Unconfirmed),
        (302, Cleanup::Unconfirmed),
        (500, Cleanup::Unconfirmed),
    ] {
        let peer = Peer::with_delete(vec![], status, Duration::ZERO).await;
        let client = http();
        let mut connection = Connection {
            http: &client,
            endpoint: &peer.endpoint,
            token: "synthetic",
            expires_at: None,
            deadline: tokio::time::Instant::now(),
            session: Some("session".parse().unwrap()),
            total: 0,
        };
        assert_eq!(connection.close().await, expected);
        assert_eq!(connection.close().await, Cleanup::NotNeeded);
        assert_eq!(peer.deletes.lock().unwrap().len(), 1);
        peer.stop().await;
    }
}

#[tokio::test]
async fn luma_mcp_cleanup_does_not_spawn_on_caller_cancellation() {
    let mut responses = replies(true);
    responses[2].hold = true;
    let peer = Peer::new(responses).await;
    let endpoint = peer.endpoint.clone();
    let task = tokio::spawn(async move {
        inspect_at(
            &http(),
            &endpoint,
            "synthetic",
            None,
            Duration::from_secs(30),
        )
        .await
    });
    peer.wait_calls(3).await;
    // Request arrival precedes body construction; wait until the held stream exists.
    tokio::time::timeout(Duration::from_secs(2), async {
        while peer.starts.load(std::sync::atomic::Ordering::SeqCst) < 3 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    peer.wait_drops(3).await;
    assert!(peer.deletes.lock().unwrap().is_empty());
    peer.stop().await;
}

#[tokio::test]
async fn luma_mcp_cleanup_never_visits_redirect_target() {
    let target = Peer::new(vec![]).await;
    let peer =
        Peer::with_redirect(replies(true), 307, Duration::ZERO, target.endpoint.clone()).await;
    assert!(inspect(&peer).await.is_ok());
    assert_eq!(peer.deletes.lock().unwrap().len(), 1);
    assert!(target.deletes.lock().unwrap().is_empty());
    assert_eq!(target.count(), 0);
    peer.stop().await;
    target.stop().await;
}

#[tokio::test]
async fn luma_mcp_cleanup_expired_token_is_not_sent() {
    use super::{cleanup::Cleanup, transport::Connection};
    let peer = Peer::new(vec![]).await;
    let client = http();
    let mut connection = Connection {
        http: &client,
        endpoint: &peer.endpoint,
        token: "synthetic",
        expires_at: Some(time::OffsetDateTime::now_utc() - time::Duration::seconds(1)),
        deadline: tokio::time::Instant::now(),
        session: Some("session".parse().unwrap()),
        total: 0,
    };
    assert_eq!(connection.close().await, Cleanup::Unconfirmed);
    assert!(peer.deletes.lock().unwrap().is_empty());
    peer.stop().await;
}

#[tokio::test]
async fn luma_mcp_cleanup_cancellation_while_delete_is_pending() {
    let peer = Peer::with_delete(replies(true), 204, Duration::from_secs(30)).await;
    let endpoint = peer.endpoint.clone();
    let task = tokio::spawn(async move {
        inspect_at(
            &http(),
            &endpoint,
            "synthetic",
            None,
            Duration::from_secs(1),
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        while peer.deletes.lock().unwrap().is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(peer.deletes.lock().unwrap().len(), 1);
    peer.stop().await;
}
