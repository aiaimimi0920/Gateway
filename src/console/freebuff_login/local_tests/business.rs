//! Real management TCP and same-root Local runtime rebuild; no catalog, login or billable request.
use super::*;
use crate::protocol::freebuff::business_target;

#[tokio::test]
async fn freebuff_business_local_tcp_commit_requires_consent_and_survives_rebuild() {
    tokio::time::timeout(Duration::from_secs(60), Box::pin(flow()))
        .await
        .unwrap();
}
async fn flow() {
    let root = std::env::temp_dir().join(format!("freebuff-business-{}", uuid::Uuid::new_v4()));
    let config = config(&root);
    let peer = Peer::start().await;
    let server = LocalServer::start(config.clone(), &peer, None).await;
    let port = server.port;
    let grant = server.grant().await;
    let document = json!({"providers":[{"id":PROVIDER,"preset":"freebuff","base_url":"https://freebuff.com",
        "explicit_credentials_only":true,"supported_models":["manual"],"credentials":[{"id":"business-account",
        "freebuff_session":{},"extra_body":{"custom":"kept"},"supported_models":["manual"]}]}],
        "model_routes":[{"pattern":"manual","provider_ids":[PROVIDER]}]});
    let patches: Vec<_> = [("api_key", "synthetic-business-token"),
        ("freebuff_session/fingerprint_id", "synthetic-business-device"),
        ("freebuff_session/fingerprint_hash", "synthetic-business-hash")]
        .into_iter().map(|(field,value)| json!({"path":format!("/providers/0/credentials/0/{field}"),"operation":"replace","value":value})).collect();
    let (status, body) = server.manage("PUT", ROUTE,
        json!({"document":document,"secretPatches":patches,"expectedRevision":server.revision()}), Some(&grant)).await;
    assert_eq!(status, 200, "{body}");
    let before = account(&server);
    let (_, public) = server.manage("GET", ROUTE, Value::Null, None).await;
    let mut draft = public["routeConfig"]["document"].clone();
    draft["providers"][0]["credentials"][0]["extra_body"]["freebuffBusiness"] =
        json!({"origin":"https://codebuff.com","client":"cli"});
    draft["providers"][0]["credentials"][0]["model_selection"] =
        json!({"fetched":["m-stable"],"manual":["manual"]});
    draft["providers"][0]["credentials"][0]["supported_models"] = json!(["manual", "m-stable"]);
    draft["providers"][0]["supported_models"] = json!(["manual", "m-stable"]);
    draft["model_routes"]
        .as_array_mut()
        .unwrap()
        .push(json!({"pattern":"m-stable","provider_ids":[PROVIDER]}));
    let input = keep_document(&server, draft);
    let old_revision = server.revision();
    let validate = format!("{ROUTE}/validate");
    assert_eq!(
        server
            .manage("POST", &validate, input.clone(), None)
            .await
            .0,
        403
    );
    let (status, body) = server.manage("PUT", ROUTE, input.clone(), None).await;
    if status != 403 {
        server.stop().await;
        peer.stop().await;
        assert_eq!(
            status, 403,
            "Business authority must require a live Secret Grant: {body}"
        );
        return;
    }
    let (status, body) = server
        .manage("PUT", ROUTE, input.clone(), Some(&grant))
        .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["committed"], true);
    assert_eq!(
        server
            .manage(
                "PUT",
                ROUTE,
                keep_document(&server, body["routeConfig"]["document"].clone()),
                None
            )
            .await
            .0,
        200
    );
    assert_public(&body, &peer);
    let after = account(&server);
    assert_eq!(after.api_key, before.api_key);
    assert_eq!(after.freebuff_session, before.freebuff_session);
    assert_eq!(after.extra_body["custom"], "kept");
    let saved = serde_json::to_value(&after).unwrap();
    assert_ne!(server.revision(), old_revision);
    assert_eq!(
        server.manage("PUT", ROUTE, input, Some(&grant)).await.0,
        409
    );
    assert_eq!(peer.counts(), (0, 0));
    // Audience changes, including opting out into legacy mode, require a live grant.
    for target in [
        Some(json!({"origin":"https://www.codebuff.com","client":"cli"})),
        Some(json!({"origin":"https://codebuff.com","client":"desktop"})),
        None,
    ] {
        let (_, public) = server.manage("GET", ROUTE, Value::Null, None).await;
        let mut draft = public["routeConfig"]["document"].clone();
        let extra = draft["providers"][0]["credentials"][0]["extra_body"]
            .as_object_mut()
            .unwrap();
        if let Some(target) = target {
            extra.insert("freebuffBusiness".into(), target);
        } else {
            extra.remove("freebuffBusiness");
        }
        let change = keep_document(&server, draft);
        for (method, path) in [("PUT", ROUTE), ("POST", validate.as_str())] {
            assert_eq!(
                server.manage(method, path, change.clone(), None).await.0,
                403
            );
            assert_eq!(
                server
                    .manage(method, path, change.clone(), Some("invalid-grant"))
                    .await
                    .0,
                403
            );
        }
        assert_eq!(serde_json::to_value(account(&server)).unwrap(), saved);
    }
    let revision = server.revision();
    server.stop().await;
    let server = LocalServer::start(config.clone(), &peer, Some(port)).await;
    assert_eq!(server.revision(), revision);
    assert_eq!(serde_json::to_value(account(&server)).unwrap(), saved);
    let candidates = server
        .state
        .route_config
        .snapshot()
        .resolve_candidates(Some("m-stable"));
    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].payload.credential_id.as_deref(),
        Some("business-account")
    );
    assert_eq!(candidates[0].payload.base_url, "https://freebuff.com");
    let target = business_target::read(candidates[0].payload.extra_body.as_ref())
        .unwrap()
        .unwrap();
    assert_eq!(target.origin.as_str(), "https://codebuff.com");
    assert_eq!(
        target.client,
        crate::protocol::freebuff::catalog::ClientSurface::Cli
    );
    assert_eq!(peer.counts(), (0, 0));
    let (_, public) = server.manage("GET", ROUTE, Value::Null, None).await;
    let mut draft = public["routeConfig"]["document"].clone();
    draft["providers"][0]["credentials"][0]["extra_body"]["freebuffBusiness"] =
        json!({"origin":"https://www.codebuff.com","client":"desktop"});
    let change = keep_document(&server, draft);
    assert_eq!(
        server
            .manage("PUT", ROUTE, change.clone(), Some(&grant))
            .await
            .0,
        403
    );
    let grant = server.grant().await;
    let (status, body) = server.manage("PUT", ROUTE, change, Some(&grant)).await;
    assert_eq!(status, 200, "{body}");
    let saved = serde_json::to_value(account(&server)).unwrap();
    let revision = server.revision();
    server.stop().await;
    let server = LocalServer::start(config, &peer, Some(port)).await;
    assert_eq!(server.revision(), revision);
    assert_eq!(serde_json::to_value(account(&server)).unwrap(), saved);
    assert_eq!(peer.counts(), (0, 0));
    server.stop().await;
    peer.stop().await;
}
