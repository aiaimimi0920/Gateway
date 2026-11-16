//! Covers real TCP grant, server-generated device, normal Keep, stale grants and durable same-root rebuild.
use super::*;

#[tokio::test]
async fn freebuff_local_tcp_save_keep_reauthorize_and_rebuild() {
    tokio::time::timeout(Duration::from_secs(90), Box::pin(flow()))
        .await
        .unwrap();
}
async fn wait_status(server: &LocalServer, path: &str, expected: &str) -> Value {
    tokio::time::timeout(Duration::from_secs(18), async {
        loop {
            let (status, body) = server.manage("GET", path, Value::Null, None).await;
            assert_eq!(status, 200, "{body}");
            if body["session"]["status"] == expected {
                return body["session"].clone();
            }
            assert!(
                !matches!(
                    body["session"]["status"].as_str(),
                    Some("failed" | "save_unconfirmed" | "expired")
                ),
                "{body}"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap()
}
fn input(server: &LocalServer, credential: Option<&str>) -> Value {
    let mut body = json!({"providerId":PROVIDER,"expectedRevision":server.revision()});
    if let Some(id) = credential {
        body["credentialId"] = json!(id);
    }
    body
}
async fn authorize(
    server: &LocalServer,
    peer: &Peer,
    grant: &str,
    credential: Option<&str>,
) -> Value {
    let (status, body) = server
        .manage("POST", AUTH, input(server, credential), Some(grant))
        .await;
    assert_eq!(status, 200, "{body}");
    assert_public(&body, peer);
    let path = format!("{AUTH}/{}", body["session"]["id"].as_str().unwrap());
    let view = wait_status(server, &path, "waiting_user").await;
    let action = json!({"expectedRevision":view["expectedRevision"]});
    assert_eq!(
        server
            .manage("POST", &format!("{path}/poll"), action.clone(), Some(grant))
            .await
            .0,
        200
    );
    assert_public(&wait_status(server, &path, "ready").await, peer);
    assert_eq!(
        server
            .manage(
                "POST",
                &format!("{path}/import"),
                action.clone(),
                Some(grant)
            )
            .await
            .0,
        200
    );
    let saved = wait_status(server, &path, "succeeded").await;
    assert_public(&saved, peer);
    // Receipt reads/repeated import do not repeat the accepted persistence or CLI request.
    assert_eq!(
        server
            .manage("POST", &format!("{path}/import"), action, Some(grant))
            .await
            .0,
        200
    );
    saved
}
async fn flow() {
    let root = std::env::temp_dir().join(format!("freebuff-local-{}", uuid::Uuid::new_v4()));
    let config = config(&root);
    let peer = Peer::start().await;
    let server = LocalServer::start(config.clone(), &peer, None).await;
    let port = server.port;
    let document = json!({"providers":[{"id":PROVIDER,"preset":"freebuff","base_url":"https://freebuff.com",
        "explicit_credentials_only":true,"credentials":[]}],"model_routes":[]});
    let (status, body) = server
        .manage(
            "PUT",
            ROUTE,
            json!({"document":document,"secretPatches":[],"expectedRevision":server.revision()}),
            None,
        )
        .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        server
            .manage("POST", AUTH, input(&server, None), None)
            .await
            .0,
        403
    );
    assert_eq!(peer.counts(), (0, 0));
    let grant = server.grant().await;
    let saved = authorize(&server, &peer, &grant, None).await;
    let id = saved["credentialId"].as_str().unwrap().to_owned();
    let device = peer.devices()[0].clone();
    assert_material(&server, 1, &device);
    let (status, public) = server.manage("GET", ROUTE, Value::Null, None).await;
    assert_eq!(status, 200);
    assert_public(&public, &peer);
    let mut document = public["routeConfig"]["document"].clone();
    document["providers"][0]["credentials"][0]["account_name"] = json!("kept-freebuff-metadata");
    document["providers"][0]["credentials"][0]["supported_models"] = json!(["contract-metadata"]);
    let (status, public) = server
        .manage("PUT", ROUTE, keep_document(&server, document), None)
        .await;
    assert_eq!(status, 200, "{public}");
    assert_public(&public, &peer);
    let revision = server.revision();
    let original = serde_json::to_value(account(&server)).unwrap();
    server.stop().await;
    assert!(root.join("state/runtime.sqlite3").is_file());
    let server = LocalServer::start(config.clone(), &peer, Some(port)).await;
    assert_eq!(server.revision(), revision);
    assert_eq!(serde_json::to_value(account(&server)).unwrap(), original);
    let stale = server
        .manage("POST", AUTH, input(&server, Some(&id)), Some(&grant))
        .await;
    assert_eq!(stale.0, 403);
    assert_eq!(peer.counts(), (1, 1));
    let fresh = server.grant().await;
    let saved = authorize(&server, &peer, &fresh, Some(&id)).await;
    assert_eq!(saved["credentialId"], id);
    assert_material(&server, 2, &device);
    assert_eq!(
        account(&server).account_name.as_deref(),
        Some("kept-freebuff-metadata")
    );
    assert_eq!(account(&server).supported_models, ["contract-metadata"]);
    let replacement = serde_json::to_value(account(&server)).unwrap();
    let revision = server.revision();
    server.stop().await;
    let server = LocalServer::start(config, &peer, Some(port)).await;
    assert_eq!(server.revision(), revision);
    assert_eq!(serde_json::to_value(account(&server)).unwrap(), replacement);
    assert_eq!(peer.devices(), [device.clone(), device]);
    assert_eq!(peer.counts(), (2, 2));
    server.stop().await;
    peer.stop().await;
}
