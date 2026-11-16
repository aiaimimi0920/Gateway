//! Synthetic protocol peer shared by management lifecycle tests; production cannot choose its origin.
use super::{Session, SessionStatus};
pub(crate) use crate::protocol::kiro_oauth::fixture::{Mock, ResponseSpec};
use crate::routing::config::RouteConfigYaml;
use serde_json::json;
use std::{sync::Arc, time::Duration};
pub(crate) fn registration() -> ResponseSpec {
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    ResponseSpec::json(
        200,
        json!({"clientId":"synthetic-console-client","clientSecret":"synthetic-console-secret",
        "clientIdIssuedAt":now,"clientSecretExpiresAt":now+86400}),
    )
}
pub(crate) fn device() -> ResponseSpec {
    ResponseSpec::json(
        200,
        json!({"deviceCode":"synthetic-device-secret","userCode":"ABCD-EFGH",
        "verificationUri":"https://device.sso.us-east-1.amazonaws.com/","expiresIn":600,"interval":1}),
    )
}
pub(crate) fn tokens(refresh: bool, expiry: bool) -> ResponseSpec {
    let mut body = json!({"accessToken":"synthetic-console-access","tokenType":"Bearer"});
    if refresh {
        body["refreshToken"] = json!("synthetic-console-refresh");
    }
    if expiry {
        body["expiresIn"] = json!(3600);
    }
    ResponseSpec::json(200, body)
}
pub(crate) fn document() -> RouteConfigYaml {
    serde_json::from_value(json!({"providers":[{"id":"kiro-pool","preset":"kiro","base_url":"https://q.us-east-1.amazonaws.com",
        "explicit_credentials_only":true,"credentials":[]},{"id":"unrelated","base_url":"https://example.invalid/v1","api_key":"synthetic-other-secret"}]})).unwrap()
}
pub(crate) fn http() -> rquest::Client {
    rquest::Client::builder().no_proxy().build().unwrap()
}
pub(crate) async fn wait(session: &Arc<Session>, expected: SessionStatus) {
    tokio::time::timeout(Duration::from_secs(12), async {
        loop {
            let current = session.view().status;
            if current == expected {
                break;
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
