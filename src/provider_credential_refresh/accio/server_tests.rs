//! Full server/Redis/PG boundaries; opt-in peers are never real provider credentials or websites.
use super::{
    server_fixture::Fixture, server_flow as flow, server_peer::Peer, server_runtime::Server,
};
use crate::console::refresh_grants::GrantPhase;
use axum::http::StatusCode;
use redis::AsyncCommands;
use serde_json::{json, Value};
use std::time::Duration;

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn accio_db_server_management_public_uses_db_successor_across_real_runtime_restart() {
    let f = Box::pin(Fixture::new()).await;
    let peer = Peer::start(Duration::ZERO, StatusCode::OK).await;
    let server = Box::pin(Server::start(&f, &peer, false)).await;
    assert_eq!(
        server
            .request(
                "POST",
                "/v1/chat/completions",
                None,
                Some(json!({"model":"claude-sonnet-4-6","messages":[]}))
            )
            .await
            .0,
        401
    );
    let (account, credential) = flow::create(&server, "2099-01-01T00:00:00Z").await;
    assert!(
        !peer.quota.lock().unwrap().is_empty(),
        "real management quota miss must use the private peer"
    );
    flow::public(&server, "chat", false, "before-rotation").await;
    flow::wire(&peer, &["server-original-access"]);
    server.manage("PUT", &format!("/v1/internal/gateway/provider-credentials/{credential}"), Some(json!({"payload":{
        "apiKey":"server-original-access","refreshToken":"server-original-refresh","refreshClientId":"explicit-client",
        "refreshEndpoint":"https://phoenix-gw.alibaba.com/api/auth/refresh_token","accioSgcookie":"server-secret-sg",
        "expiresAt":"2000-01-01T00:00:00.123456789Z","headers":{"utdid":"server-device"},"supportedModels":["claude-sonnet-4-6"]}}))).await;
    server.stop().await;
    let server = Box::pin(Server::start(&f, &peer, true)).await;
    f.wait_phase(GrantPhase::Published).await;
    assert_eq!(
        peer.refresh.lock().unwrap().as_slice(),
        &[json!({"accessToken":"server-original-access","refreshToken":"server-original-refresh"})]
    );
    let pool = server.state.redis_pool.clone();
    let key = crate::redis::keys::credential_key(&credential);
    let mut conn = pool.get().await.unwrap();
    let sentinel = "{\"hostedIdentity\":\"not-a-db-credential\"}";
    let _: () = conn.set(&key, sentinel).await.unwrap();
    for kind in ["chat", "responses", "messages"] {
        for stream in [false, true] {
            flow::public(&server, kind, stream, &format!("after-{kind}-{stream}")).await;
        }
    }
    assert_eq!(conn.get::<_, String>(&key).await.unwrap(), sentinel);
    let _: usize = conn.del(&key).await.unwrap();
    drop(conn);
    flow::wire(
        &peer,
        &[
            "server-original-access",
            "server-rotated-access",
            "server-rotated-access",
            "server-rotated-access",
            "server-rotated-access",
            "server-rotated-access",
            "server-rotated-access",
        ],
    );
    flow::audit(&f, &account, 7).await;
    let view = crate::db::get_provider_credential(&f.pg, &credential)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(view.payload["expiresAt"], "2099-01-01T00:00:00.123456789Z");
    assert_eq!(view.payload["accioSgcookie"], "server-secret-sg");
    let management = server
        .manage(
            "GET",
            &format!("/v1/internal/gateway/provider-credentials/{credential}"),
            None,
        )
        .await;
    assert!(
        !management.to_string().contains("server-secret-sg")
            && !management.to_string().contains("server-rotated-refresh")
    );
    server.stop().await;
    let restarted = Box::pin(Server::start(&f, &peer, true)).await;
    flow::public(&restarted, "chat", false, "after-third-server").await;
    assert_eq!(peer.refresh.lock().unwrap().len(), 1);
    restarted.stop().await;
    peer.stop().await;
    f.finish().await;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn accio_db_server_shutdown_joins_original_refresh_and_restart_does_not_resend() {
    let f = Box::pin(Fixture::new()).await;
    let peer = Peer::start(Duration::from_millis(500), StatusCode::OK).await;
    let seed = Box::pin(Server::start(&f, &peer, false)).await;
    flow::create(&seed, "2000-01-01T00:00:00Z").await;
    seed.stop().await;
    let server = Box::pin(Server::start(&f, &peer, true)).await;
    peer.wait_refresh().await;
    crate::runtime::mark_runtime_draining(&server.state, "server-refresh-drain");
    assert!(server.state.lifecycle.accio_db_refresh_stop.is_requested());
    server.stop().await;
    assert_eq!(f.phase().await, Some(GrantPhase::Published));
    let restarted = Box::pin(Server::start(&f, &peer, true)).await;
    flow::public(&restarted, "responses", true, "after-drain").await;
    assert_eq!(peer.refresh.lock().unwrap().len(), 1);
    restarted.stop().await;
    peer.stop().await;
    f.finish().await;
}

#[tokio::test]
#[ignore = "requires dedicated guarded PostgreSQL and Redis; serialize tests"]
async fn accio_db_server_unconfirmed_refresh_keeps_permanent_sending_after_restart() {
    let f = Box::pin(Fixture::new()).await;
    let peer = Peer::start(Duration::ZERO, StatusCode::INTERNAL_SERVER_ERROR).await;
    let seed = Box::pin(Server::start(&f, &peer, false)).await;
    let (_, credential) = flow::create(&seed, "2000-01-01T00:00:00Z").await;
    seed.stop().await;
    let server = Box::pin(Server::start(&f, &peer, true)).await;
    peer.wait_refresh().await;
    server.stop().await;
    assert_eq!(f.phase().await, Some(GrantPhase::Sending));
    let restarted = Box::pin(Server::start(&f, &peer, true)).await;
    let _: Value = restarted
        .manage("GET", "/v1/internal/gateway/provider-credentials", None)
        .await;
    restarted.stop().await;
    assert_eq!(peer.refresh.lock().unwrap().len(), 1);
    assert_eq!(
        crate::db::get_provider_credential(&f.pg, &credential)
            .await
            .unwrap()
            .unwrap()
            .payload["apiKey"],
        "server-original-access"
    );
    assert!(peer.inference.lock().unwrap().is_empty());
    peer.stop().await;
    f.finish().await;
}
