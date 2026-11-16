//! Closing a real caller socket must drop inference before neutral refund/audit settlement.
use super::{
    requests, run,
    runtime::Fixture,
    upstream::{Mode, DEADLINE},
};
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};

async fn verify(protocol: u8, stream: bool) {
    let modes = &[
        Mode::BeforeHeaders,
        Mode::BeforeBody,
        Mode::PartialFrame,
        Mode::Partial,
        Mode::Reasoning,
        Mode::ToolArguments,
        Mode::BeforeEof,
    ];
    for &mode in modes {
        let fixture = Fixture::start(mode).await;
        let mut socket = TcpStream::connect(fixture.local.origin.trim_start_matches("http://"))
            .await
            .unwrap();
        let body = requests::request(protocol, &[], stream).to_string();
        let request = format!("POST {} HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nCookie: caller-must-not-forward\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}", requests::path(protocol, stream), fixture.local.origin.trim_start_matches("http://"), fixture.token, body.len(), body);
        socket.write_all(request.as_bytes()).await.unwrap();
        fixture.upstream.wait().await;
        assert_eq!(
            fixture.balance().await,
            9,
            "one Local message reserved before inference"
        );
        if stream
            && matches!(
                mode,
                Mode::Partial | Mode::Reasoning | Mode::ToolArguments | Mode::BeforeEof
            )
        {
            let marker = if mode == Mode::Partial {
                b"before".to_vec()
            } else if mode == Mode::Reasoning {
                b"reasoningContent".to_vec()
            } else if mode == Mode::ToolArguments {
                b"\\\"v\\\":".to_vec()
            } else if protocol == 3 {
                // Native success events are deliberately held until true EOF. Observe the last
                // nonterminal block instead, while the complete upstream sequence remains held.
                super::wire::event(
                    "contentBlockStop",
                    serde_json::json!({"contentBlockIndex":4}),
                )
            } else {
                b"metadata".to_vec()
            };
            let mut received = Vec::new();
            timeout(DEADLINE, async {
                while !received
                    .windows(marker.len())
                    .any(|bytes| bytes == marker.as_slice())
                {
                    let mut bytes = [0; 4096];
                    let size = socket.read(&mut bytes).await.unwrap();
                    assert!(size > 0, "response ended before cancellation barrier");
                    received.extend_from_slice(&bytes[..size]);
                    assert!(received.len() < 256 * 1024);
                }
            })
            .await
            .unwrap();
            assert!(received.starts_with(b"HTTP/1.1 200"));
            let text = String::from_utf8_lossy(&received);
            assert!(
                !text.contains("response.completed")
                    && !text.contains("message_stop")
                    && !text.contains("[DONE]")
            );
            if protocol == 3 {
                assert!(!text.contains("messageStop") && !text.contains("metadata"));
            }
        }
        let running = fixture
            .state
            .local_runtime
            .as_ref()
            .unwrap()
            .list_audits(&neuro_gateway::db::RequestAuditFilters::default())
            .await
            .unwrap();
        assert_eq!(running.len(), 1);
        assert_eq!(
            running[0].status, "running",
            "no success before real upstream EOF"
        );
        drop(socket);
        fixture.upstream.released().await;
        let rows = fixture.settled(1, "cancelled").await;
        assert!(rows[0].total_tokens.is_none());
        let reason = rows[0].route_trace.as_ref().unwrap()["terminationReason"]
            .as_str()
            .unwrap();
        assert!(
            matches!(reason, "request_consumer_closed" | "stream_consumer_closed"),
            "{protocol}/{stream}/{mode:?}: {reason}"
        );
        fixture.neutral();
        // A quiescence window detects a late retry or duplicate terminal write, not synchronization.
        tokio::time::sleep(Duration::from_millis(30)).await;
        fixture.settled(1, "cancelled").await;
        fixture.neutral();
        fixture.close().await;
    }
}
macro_rules! cancellation {
    ($name:ident, $protocol:expr, $stream:expr) => {
        #[test]
        fn $name() {
            run(|| verify($protocol, $stream));
        }
    };
}
cancellation!(chat_buffered_tcp_disconnect_cancels_and_refunds, 0, false);
cancellation!(
    messages_buffered_tcp_disconnect_cancels_and_refunds,
    1,
    false
);
cancellation!(
    responses_buffered_tcp_disconnect_cancels_and_refunds,
    2,
    false
);
cancellation!(native_buffered_tcp_disconnect_cancels_and_refunds, 3, false);
cancellation!(chat_sse_tcp_disconnect_at_each_boundary_is_neutral, 0, true);
cancellation!(
    messages_sse_tcp_disconnect_at_each_boundary_is_neutral,
    1,
    true
);
cancellation!(
    responses_sse_tcp_disconnect_at_each_boundary_is_neutral,
    2,
    true
);
cancellation!(
    native_eventstream_tcp_disconnect_at_each_boundary_is_neutral,
    3,
    true
);
