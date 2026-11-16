//! Public client must expose replaceable snapshots while the native turn is still in flight.
use super::*;

#[tokio::test]
async fn perplexity_web_stream_exposes_snapshot_before_terminal_without_standard_text() {
    let raw = serde_json::json!({"blocks":[{"intended_usage":"ask_text","markdown_block":{"answer":"临时答案"}}],"read_write_token":"private-never-forward"});
    let server = wire::Server::start(format!("data: {raw}\n\n"), 200, true).await;
    let client = client(&server);
    let response = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        wire::within(
            &server.origin,
            client.execute_stream(&payload(), &request(), "turbo", None),
        ),
    )
    .await
    .expect("Headers and native snapshots must not wait for the terminal event")
    .unwrap();
    let UpstreamStreamingResponse::NativeEvents(mut stream) = response else {
        panic!("Expected the product-owned live source, not pre-aggregated bytes")
    };
    let bytes = tokio::time::timeout(std::time::Duration::from_secs(1), stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(
        text.contains("gateway_perplexity_snapshot") && text.contains("临时答案"),
        "{text}"
    );
    assert!(!text.contains("private-never-forward") && !text.contains("[DONE]"));
    assert!(!text.contains("\"content\":") && !text.contains("\"finish_reason\":\"stop\""));
    drop(stream);
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while server.closed() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(server.attempts(), 1);
}
