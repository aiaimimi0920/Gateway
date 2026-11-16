//! Real authorization callback/import, existing scheduler, public request settlement, and same-directory rebuilds.
use super::*;
use crate::access_balance::AccessBalanceStore;

#[tokio::test]
async fn vertex_oauth_local_tcp_authorize_refresh_reauthorize_and_rebuild() {
    tokio::time::timeout(Duration::from_secs(120), Box::pin(local_flow()))
        .await
        .unwrap();
}
pub(super) fn login(server: &LocalServer, credential: Option<&str>, private: bool) -> Value {
    let mut input = json!({"providerId":PROVIDER,"expectedRevision":server.revision(),
        "clientId":if private { CLIENT } else { "synthetic-public-client" },"target":target()});
    if private {
        input["clientSecret"] = json!(SECRET);
    }
    if let Some(id) = credential {
        input["credentialId"] = json!(id);
    }
    input
}
pub(super) async fn authorize(server: &LocalServer, grant: &str, input: Value) -> Value {
    let (status, view) = server.manage("POST", AUTH, input, Some(grant)).await;
    assert_eq!(status, 200, "{view}");
    let view = &view["session"];
    let auth = url::Url::parse(view["authorizationUrl"].as_str().unwrap()).unwrap();
    let query: std::collections::HashMap<_, _> = auth.query_pairs().into_owned().collect();
    let mut callback = url::Url::parse(view["redirectUri"].as_str().unwrap()).unwrap();
    callback
        .query_pairs_mut()
        .append_pair("code", "synthetic-code")
        .append_pair("state", &query["state"]);
    let reply = rquest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap()
        .get(callback.as_str())
        .send()
        .await
        .unwrap();
    assert_eq!(reply.status(), 200);
    let path = format!("{AUTH}/{}", view["id"].as_str().unwrap());
    let ready = wait_status(server, &path, "ready").await;
    assert!(!ready.to_string().contains(SECRET));
    let body = json!({"expectedRevision":view["expectedRevision"]});
    let (status, _) = server
        .manage("POST", &format!("{path}/import"), body, Some(grant))
        .await;
    assert_eq!(status, 200);
    wait_status(server, &path, "succeeded").await
}
async fn wait_status(server: &LocalServer, path: &str, status: &str) -> Value {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let (code, body) = server.manage("GET", path, Value::Null, None).await;
            assert_eq!(code, 200, "{body}");
            if body["session"]["status"] == status {
                return body["session"].clone();
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}
async fn balance(server: &LocalServer, id: &str) -> i64 {
    AccessBalanceStore::from_state(&server.state)
        .unwrap()
        .get(id)
        .await
        .unwrap()
        .unwrap()
        .remaining_messages
        .unwrap()
}
async fn local_flow() {
    let root = std::env::temp_dir().join(format!("vertex-oauth-local-{}", uuid::Uuid::new_v4()));
    let config = config(&root);
    let upstream = wire::Server::start(200, upstream_body()).await;
    let issuer = Issuer::start().await;
    let server = LocalServer::start(config.clone(), &upstream, &issuer, None).await;
    let port = server.port;
    let body = json!({"document":{"providers":[{"id":PROVIDER,"preset":"google-agent-platform-official-api",
        "auth_mode":"vertex_user_oauth","base_url":BASE,"explicit_credentials_only":true,"credentials":[]}],"model_routes":[]},
        "secretPatches":[],"expectedRevision":server.revision()});
    let (status, saved) = server.manage("PUT", ROUTE, body, None).await;
    assert_eq!(status, 200, "{saved}");
    let grant = server.grant().await;
    let original = login(&server, None, true);
    assert_eq!(
        server.manage("POST", AUTH, original.clone(), None).await.0,
        403
    );
    let saved = authorize(&server, &grant, original).await;
    let id = saved["credentialId"].as_str().unwrap().to_owned();
    let before = account(&server);
    assert_eq!(before.api_key.as_deref(), Some("synthetic-access-1"));
    let mut document =
        server.manage("GET", ROUTE, Value::Null, None).await.1["routeConfig"]["document"].clone();
    document["providers"][0]["credentials"][0]["supported_models"] =
        json!([MODEL, "gemini-metadata"]);
    document["providers"][0]["credentials"][0]["account_name"] = json!("kept-local-metadata");
    let (status, saved) = server
        .manage("PUT", ROUTE, keep_document(&server, document), None)
        .await;
    assert_eq!(status, 200, "{saved}");
    assert!(!saved.to_string().contains(SECRET));
    let created = server.manage("POST", "/v1/internal/gateway/access/keys",
        json!({"ownerType":"user","ownerId":"vertex-local-fixture","resolvedProjectId":"local","resolvedTenantId":"local",
        "keyKind":"normal","publicKeyPrefix":"sk-gw","displayName":"Full Vertex local","metadata":{"models":[MODEL]}}), None).await;
    assert_eq!(created.0, 200, "{}", created.1);
    let key_id = created.1["id"].as_str().unwrap();
    let token = created.1["token"].as_str().unwrap();
    assert_eq!(
        server
            .manage(
                "POST",
                &format!("/v1/internal/gateway/access/keys/{key_id}/balances/adjust"),
                json!({"balanceMode":"message_prepaid","totalMessages":4,"remainingMessages":4}),
                None
            )
            .await
            .0,
        200
    );
    let (status, text) = server.call(token, false).await;
    assert_eq!(status, 200, "{text}");
    assert!(text.contains("local-vertex-answer"));
    assert_eq!(balance(&server, key_id).await, 3);
    server.start_refresh(&issuer);
    wait_access(&server, REFRESHED_ACCESS).await;
    let refreshed = account(&server);
    assert_eq!(refreshed.id.as_deref(), Some(id.as_str()));
    assert_eq!(refreshed.refresh_token.as_deref(), Some(REFRESHED_TOKEN));
    assert_eq!(
        refreshed.account_name.as_deref(),
        Some("kept-local-metadata")
    );
    let revision = server.revision();
    server.stop().await;
    let server = LocalServer::start(config.clone(), &upstream, &issuer, Some(port)).await;
    assert_eq!(server.revision(), revision);
    assert_eq!(
        serde_json::to_value(account(&server)).unwrap(),
        serde_json::to_value(refreshed).unwrap()
    );
    assert_eq!(balance(&server, key_id).await, 3);
    let (status, text) = server.call(token, true).await;
    assert_eq!(status, 200, "{text}");
    assert!(text.contains("local-vertex-answer") && text.contains("[DONE]"));
    let replacement = login(&server, Some(&id), false);
    assert_eq!(
        server
            .manage("POST", AUTH, replacement.clone(), Some(&grant))
            .await
            .0,
        403
    );
    let fresh_grant = server.grant().await;
    let saved = authorize(&server, &fresh_grant, replacement).await;
    assert_eq!(saved["credentialId"], id);
    assert_eq!(account(&server).id.as_deref(), Some(id.as_str()));
    let context = serde_json::to_value(account(&server).vertex_oauth).unwrap();
    assert_eq!(context["client_id"], "synthetic-public-client");
    assert!(context.get("client_secret").is_none_or(Value::is_null));
    assert_eq!(context["target"], target());
    let revision = server.revision();
    server.stop().await;
    let server = LocalServer::start(config, &upstream, &issuer, Some(port)).await;
    assert_eq!(server.revision(), revision);
    assert_eq!(account(&server).id.as_deref(), Some(id.as_str()));
    assert_eq!(
        account(&server).supported_models,
        vec![MODEL, "gemini-metadata"]
    );
    assert_eq!(balance(&server, key_id).await, 2);
    let (status, text) = server.call(token, false).await;
    assert_eq!(status, 200, "{text}");
    assert!(text.contains("local-vertex-answer"));
    assert_eq!(balance(&server, key_id).await, 1);
    assert_wire(
        &upstream,
        &["synthetic-access-1", REFRESHED_ACCESS, "synthetic-access-2"],
    );
    issuer.assert_grants();
    server.stop().await;
    upstream.stop().await;
    issuer.stop().await;
}
