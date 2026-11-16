//! Reuse the real protocol's loopback fixture; never override origins outside test builds.
use super::{Binding, Session, SessionStatus};
pub(crate) use crate::protocol::freebuff_login::fixture::{code, user, Mock, Spec};
use crate::{
    protocol::freebuff_login::{LoginTransport, TEST_ORIGIN},
    routing::config::RouteConfigYaml,
    state::AppState,
};
use serde_json::json;
use std::{sync::Arc, time::Duration};
pub(crate) const FINGERPRINT: &str = "enhanced-synthetic-device";
pub(crate) fn document() -> RouteConfigYaml {
    serde_json::from_value(json!({"providers":[{"id":"freebuff-pool","preset":"freebuff","base_url":"https://freebuff.com",
        "explicit_credentials_only":true,"credentials":[]},
        {"id":"unrelated","base_url":"https://example.invalid/v1","api_key":"synthetic-other-secret"}]})).unwrap()
}
pub(crate) fn http() -> LoginTransport {
    LoginTransport::new().unwrap()
}
pub(crate) async fn wait(session: &Arc<Session>, expected: SessionStatus) {
    tokio::time::timeout(Duration::from_secs(18), async {
        loop {
            let current = session.view().status;
            if current == expected {
                return;
            }
            assert!(
                !matches!(
                    current,
                    SessionStatus::Failed | SessionStatus::Cancelled | SessionStatus::Expired
                ),
                "unexpected {current:?}"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}
pub(crate) async fn ready(state: &Arc<AppState>) -> Arc<Session> {
    let mock = Mock::new(vec![code(), user()]).await;
    let session = TEST_ORIGIN
        .scope(mock.origin.clone(), async {
            state
                .lifecycle
                .freebuff_login
                .start(
                    Binding::new(
                        "synthetic-owner",
                        "freebuff-pool",
                        state.route_config.snapshot().revision().id(),
                    )
                    .unwrap(),
                    http(),
                    FINGERPRINT.into(),
                )
                .unwrap()
        })
        .await;
    wait(&session, SessionStatus::WaitingUser).await;
    TEST_ORIGIN
        .scope(mock.origin.clone(), async {
            state
                .lifecycle
                .freebuff_login
                .start_poll(session.id(), session.binding(), http())
                .unwrap();
        })
        .await;
    wait(&session, SessionStatus::Ready).await;
    mock.stop().await;
    session
}
