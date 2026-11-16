//! Actual management TCP and complete Local bootstrap rebuild, without any browser/API response mock.
use super::*;
#[tokio::test]
async fn freebuff_claim_management_real_tcp_final_history_and_same_root_rebuild() {
    tokio::time::timeout(Duration::from_secs(90), Box::pin(flow()))
        .await
        .unwrap();
}
async fn flow() {
    let root = std::env::temp_dir().join(format!(
        "gateway-local-freebuff-claim-tcp-{}",
        uuid::Uuid::new_v4()
    ));
    let config = config(&root);
    let peer = wire::Peer::session(json!({"status":"ended","freebucksRefund":0}), 200, None).await;
    let server = Host::start(config.clone(), &peer, None).await;
    let port = server.port;
    let id = server.seed_original_claim(&peer).await;
    let grant = server.grant().await;
    assert_eq!(server.counts().await, (1, 0));
    let (status, journal) = server.request(ENDPOINT, server.input(), Some(&grant)).await;
    assert_eq!(status, 200, "{journal}");
    assert_eq!(journal["claim"]["attemptId"], id);
    assert!(peer.hits.lock().unwrap().is_empty());
    let mut input = server.input();
    input["attemptId"] = json!(id);
    let (status, ended) = server
        .request(&format!("{ENDPOINT}/end"), input.clone(), Some(&grant))
        .await;
    assert_eq!(status, 200, "{ended}");
    assert_eq!(ended["localReleased"], true);
    assert_eq!(ended["receipt"]["refund"], 0.0);
    assert_eq!(ended["receipt"]["refundSources"], Value::Null);
    assert_eq!(ended["fromHistory"], false);
    assert_eq!(server.counts().await, (0, 1));
    assert_end_wire(&peer, &id);
    let document = serde_json::to_value(server.state.route_config.snapshot().document()).unwrap();
    let revision = server.revision();
    server.stop().await;
    let server = Host::start(config, &peer, Some(port)).await;
    assert_eq!(server.revision(), revision);
    assert_eq!(server.port, port);
    assert_eq!(
        serde_json::to_value(server.state.route_config.snapshot().document()).unwrap(),
        document
    );
    assert_eq!(
        server
            .request(ENDPOINT, server.input(), Some(&grant))
            .await
            .0,
        403
    );
    let grant = server.grant().await;
    let (status, journal) = server.request(ENDPOINT, server.input(), Some(&grant)).await;
    assert_eq!(status, 200, "{journal}");
    assert!(journal["claim"].is_null());
    assert_eq!(journal["history"].as_array().unwrap().len(), 1);
    assert_eq!(journal["history"][0]["attemptId"], id);
    assert_eq!(journal["history"][0]["receipt"]["refund"], 0.0);
    let (status, reconciled) = server
        .request(&format!("{ENDPOINT}/end"), input, Some(&grant))
        .await;
    assert_eq!(status, 200, "{reconciled}");
    assert_eq!(reconciled["fromHistory"], true);
    assert_eq!(server.counts().await, (0, 1));
    assert_end_wire(&peer, &id);
    server.stop().await;
    peer.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}
