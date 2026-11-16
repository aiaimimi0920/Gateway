//! The accepted save guard exists before the first poll and its deadline includes publication wait.
use super::*;
use crate::{
    protocol::accio_oauth::{LoginSite, OAuthClient},
    routing::config::RouteConfigYaml,
};
use serde_json::json;
use std::{sync::Arc, time::Duration};
#[path = "../../../tests/console_contract_support/mod.rs"]
mod support;

async fn ready() -> (support::ConsoleStateFixture, Arc<Session>) {
    let document: RouteConfigYaml = serde_json::from_value(json!({"providers":[{"id":"pool","preset":"accio",
        "base_url":"https://phoenix-gw.alibaba.com","explicit_credentials_only":true,"credentials":[]}]})).unwrap();
    let fixture = support::ConsoleStateFixture::new(document, true);
    let binding = Binding::new(
        "synthetic-owner",
        "pool",
        fixture.state.route_config.snapshot().revision().id(),
    )
    .unwrap();
    let session = fixture
        .state
        .lifecycle
        .accio_oauth
        .create(
            binding,
            OAuthClient::new("synthetic-client").unwrap(),
            LoginSite::Global,
            34567,
            "synthetic-device".into(),
        )
        .unwrap();
    let url = url::Url::parse(&session.view().authorization_url.unwrap()).unwrap();
    assert_eq!(url.host_str(), Some("zh.accio.com"));
    let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
    let callback = format!(
        "{}?code=synthetic-code&state={}",
        query["return_url"], query["state"]
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/token", listener.local_addr().unwrap());
    let app = axum::Router::new().route("/token", axum::routing::post(|| async {
        axum::Json(json!({"success":true,"data":{"accessToken":"synthetic-access","refreshToken":"synthetic-refresh","expiresAt":"2099-01-01T00:00:00.123456789Z","sgcookie":"synthetic-sg"}}))
    }));
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let result = crate::protocol::accio_oauth::TEST_TOKEN_ENDPOINT
        .scope(
            endpoint,
            session.complete(
                &rquest::Client::builder().no_proxy().build().unwrap(),
                &callback,
            ),
        )
        .await;
    task.abort();
    let _ = task.await;
    result.unwrap();
    (fixture, session)
}

#[tokio::test]
async fn unpolled_save_drop_is_unconfirmed_and_does_not_reopen_admission() {
    let (fixture, session) = ready().await;
    let operation = session
        .begin_save(&fixture.state, session.binding())
        .unwrap()
        .unwrap();
    let target = session.view().target_credential_id.unwrap();
    let future = operation.run(fixture.state.clone(), session.clone());
    drop(future);
    assert_eq!(session.view().status, SessionStatus::SaveUnconfirmed);
    assert_eq!(
        session.view().target_credential_id.as_deref(),
        Some(target.as_str())
    );
    assert!(session.view().credential_id.is_none());
    assert!(session.view().saved_revision.is_none());
    assert!(session
        .begin_save(&fixture.state, session.binding())
        .unwrap()
        .is_none());
    assert!(
        fixture.state.route_config.snapshot().document().providers[0]
            .credentials
            .is_empty()
    );
    fixture.state.lifecycle.accio_oauth.join().await;
}

#[tokio::test]
async fn save_deadline_includes_publication_wait_and_never_renews_or_retries_material() {
    let (fixture, session) = ready().await;
    let original = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_owned();
    let lock = fixture
        .state
        .lifecycle
        .health_control
        .publication
        .lock()
        .await;
    let operation = session
        .begin_save(&fixture.state, session.binding())
        .unwrap()
        .unwrap();
    tokio::time::timeout(
        Duration::from_secs(25),
        operation.run(fixture.state.clone(), session.clone()),
    )
    .await
    .unwrap();
    assert_eq!(session.view().status, SessionStatus::SaveUnconfirmed);
    assert_eq!(session.view().error_code, Some("accio_login_save_timeout"));
    assert!(session
        .begin_save(&fixture.state, session.binding())
        .unwrap()
        .is_none());
    drop(lock);
    assert_eq!(
        fixture.state.route_config.snapshot().revision().id(),
        original
    );
    assert!(
        fixture.state.route_config.snapshot().document().providers[0]
            .credentials
            .is_empty()
    );
    fixture.state.lifecycle.accio_oauth.join().await;
}
