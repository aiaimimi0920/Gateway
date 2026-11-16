//! Disposable normal-main browser host; file controls are test-only and never HTTP management routes.
use super::*;

#[tokio::test]
#[ignore = "Requires an explicit new fixture directory and dedicated browser"]
async fn vertex_oauth_local_ui_fixture() {
    tokio::time::timeout(Duration::from_secs(2400), Box::pin(serve_browser()))
        .await
        .unwrap();
}
fn ready(root: &Path, server: &LocalServer, generation: u64, sequence: u64) {
    let body = json!({"origin":server.origin,"port":server.port,"pid":std::process::id(),
        "revision":server.revision(),"generation":generation,"sequence":sequence,"storageMode":"local"});
    let staged = root.join("ready.next.json");
    std::fs::write(&staged, serde_json::to_vec_pretty(&body).unwrap()).unwrap();
    std::fs::rename(staged, root.join("ready.json")).unwrap();
}
async fn serve_browser() {
    let root = std::path::PathBuf::from(
        std::env::var_os("GATEWAY_VERTEX_OAUTH_UI_ROOT").expect("Explicit new root required"),
    );
    assert!(root.is_absolute() && !root.exists());
    let config = config(&root);
    let upstream = wire::Server::start(200, upstream_body()).await;
    let issuer = Issuer::start().await;
    let mut server = LocalServer::start(config.clone(), &upstream, &issuer, None).await;
    let (mut generation, mut sequence) = (0_u64, 0_u64);
    let mut identity = None;
    let mut boots = Vec::new();
    ready(&root, &server, generation, sequence);
    loop {
        tokio::time::sleep(Duration::from_millis(200)).await;
        let command = match std::fs::read(root.join("command.json")) {
            Ok(bytes) => serde_json::from_slice::<Value>(&bytes).unwrap(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => panic!("Fixture command read failed: {e}"),
        };
        let next = command["sequence"].as_u64().unwrap();
        if next <= sequence {
            continue;
        }
        sequence = next;
        match command["action"].as_str().unwrap() {
            "refresh" => {
                assert_eq!(issuer.counts(), (1, 0));
                identity = account(&server).id;
                server.start_refresh(&issuer);
                wait_access(&server, REFRESHED_ACCESS).await;
                assert_eq!(issuer.counts(), (1, 1));
            }
            "restart" => {
                let revision = server.revision();
                let saved = serde_json::to_value(account(&server)).unwrap();
                let port = server.port;
                server.stop().await;
                server = LocalServer::start(config.clone(), &upstream, &issuer, Some(port)).await;
                generation += 1;
                assert_eq!(server.revision(), revision);
                assert_eq!(serde_json::to_value(account(&server)).unwrap(), saved);
                assert_eq!(account(&server).id.as_deref(), identity.as_deref());
                boots.push(json!({"generation":generation,"revision":revision,"credential":identity,
                    "target":target(),"clientSecretPresent":saved["vertex_oauth"]["client_secret"].is_string()}));
            }
            "finish" => break,
            action => panic!("Unknown fixture command: {action}"),
        }
        ready(&root, &server, generation, sequence);
    }
    assert_eq!(generation, 2);
    assert_eq!(account(&server).id.as_deref(), identity.as_deref());
    let saved = serde_json::to_value(account(&server)).unwrap();
    assert_eq!(
        saved["vertex_oauth"]["client_id"],
        "synthetic-public-client"
    );
    assert!(saved["vertex_oauth"]
        .get("client_secret")
        .is_none_or(Value::is_null));
    assert_wire(
        &upstream,
        &["synthetic-access-1", REFRESHED_ACCESS, "synthetic-access-2"],
    );
    issuer.assert_grants();
    server.stop().await;
    upstream.stop().await;
    issuer.stop().await;
    std::fs::write(root.join("completed.json"), serde_json::to_vec_pretty(&json!({"boots":boots,
        "upstreamCalls":3,"codeGrants":2,"refreshGrants":1,"stableCredential":identity,"publicClientSecretCleared":true})).unwrap()).unwrap();
}
