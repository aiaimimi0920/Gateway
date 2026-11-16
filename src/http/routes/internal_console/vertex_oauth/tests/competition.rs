//! Callback and manual completion share one consume gate; stop also interrupts half-read sockets.
use super::{fixture::*, *};
use crate::console::vertex_oauth::SessionStatus;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn vertex_oauth_management_manual_and_listener_competition_cannot_exchange_twice() {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let mock = Mock::new(Reply {
        body: tokens().to_string(),
        header_delay: Duration::from_millis(100),
        ..Reply::default()
    })
    .await;
    let view = create(state, &grant, create_body(state), &mock).await;
    let session = session(state, &view);
    let mut stream = connect(&view).await;
    stream
        .write_all(callback_request(&view).await.as_bytes())
        .await
        .unwrap();
    mock.wait_called().await;
    assert_eq!(session.view().status, SessionStatus::Exchanging);
    assert_eq!(
        action(
            state,
            &view,
            Some(&grant),
            "complete",
            Some(&callback(&view))
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    wait_status(&session, SessionStatus::Ready).await;
    assert_eq!(mock.count(), 1);
    state.lifecycle.vertex_oauth.join().await;
    mock.stop().await;
}

#[tokio::test]
async fn vertex_oauth_management_manual_completion_stops_listener_mid_header_without_waiting_read_budget(
) {
    let fixture = support::ConsoleStateFixture::new(document(), true);
    let state = &fixture.state;
    let grant = support::grant_console_secret_access(state).await;
    let mock = issuer().await;
    let view = create(state, &grant, create_body(state), &mock).await;
    let session = session(state, &view);
    let mut stream = connect(&view).await;
    stream
        .write_all(b"GET /?code=partial HTTP/1.1\r\nHost: ")
        .await
        .unwrap();
    tokio::task::yield_now().await;
    complete(state, &view, &grant, &mock).await;
    assert_eq!(session.view().status, SessionStatus::Ready);
    let mut byte = [0];
    let closed = tokio::time::timeout(Duration::from_secs(2), stream.read(&mut byte))
        .await
        .unwrap();
    assert!(matches!(closed, Ok(0)) || closed.is_err());
    assert_eq!(mock.count(), 1);
    state.lifecycle.vertex_oauth.join().await;
    mock.stop().await;
}
