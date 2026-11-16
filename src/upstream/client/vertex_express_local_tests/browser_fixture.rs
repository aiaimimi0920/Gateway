//! Explicit disposable host for normal main UI testing; no production test/control endpoint.
use super::{assert_wire, config, upstream_body, wire, LocalServer};
use serde_json::json;
use std::{path::Path, time::Duration};

#[tokio::test]
#[ignore = "Requires an explicit new fixture directory and a dedicated browser session"]
async fn vertex_express_local_ui_fixture() {
    tokio::time::timeout(Duration::from_secs(1800), Box::pin(serve_browser()))
        .await
        .unwrap();
}
fn write_ready(root: &Path, server: &LocalServer, generation: u64) {
    let value = json!({"origin":server.origin,"port":server.port,"generation":generation,
        "pid":std::process::id(),"revision":server.revision(),"storageMode":"local"});
    let staged = root.join("ready.next.json");
    std::fs::write(&staged, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    std::fs::rename(staged, root.join("ready.json")).unwrap();
}
async fn serve_browser() {
    let root = std::path::PathBuf::from(
        std::env::var_os("GATEWAY_VERTEX_EXPRESS_UI_ROOT").expect("Explicit fixture root required"),
    );
    assert!(
        root.is_absolute() && !root.exists(),
        "Fixture root must be a new absolute path"
    );
    let config = config(&root);
    let upstream = wire::Server::start(200, upstream_body()).await;
    let mut server = LocalServer::start(config.clone(), &upstream, None).await;
    let mut generation = 0_u64;
    let mut consumed = 0_u64;
    let mut boots = Vec::new();
    write_ready(&root, &server, generation);
    loop {
        tokio::time::sleep(Duration::from_millis(200)).await;
        let command = match std::fs::read(root.join("command.json")) {
            Ok(bytes) => serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => panic!("Fixture command read failed: {error}"),
        };
        let sequence = command["sequence"].as_u64().unwrap();
        if sequence <= consumed {
            continue;
        }
        consumed = sequence;
        match command["action"].as_str().unwrap() {
            "restart" => {
                let port = server.port;
                let revision = server.revision();
                server.stop().await;
                server = LocalServer::start(config.clone(), &upstream, Some(port)).await;
                assert_eq!(server.revision(), revision);
                generation += 1;
                let providers = server.state.route_config.get_providers();
                assert_eq!(providers[0].id, "vertex-express-api");
                assert_eq!(providers[0].credential_pool[0].id, "ui-key");
                assert_eq!(
                    providers[0].credential_pool[0].payload.api_key == super::KEY,
                    generation == 1
                );
                boots.push(json!({"generation":generation,"revision":revision,
                    "providers":providers.len(),"keyPresent":providers[0].credential_pool[0].payload.api_key == super::KEY}));
                write_ready(&root, &server, generation);
            }
            "finish" => break,
            action => panic!("Unknown fixture command: {action}"),
        }
    }
    assert_eq!(
        generation, 2,
        "Verify saved Key and clear across separate same-port runtime rebuilds"
    );
    assert_wire(&upstream);
    let providers = server.state.route_config.get_providers();
    assert_eq!(providers[0].id, "vertex-express-api");
    assert_eq!(providers[0].credential_pool[0].id, "ui-key");
    assert!(providers[0].credential_pool[0].payload.api_key.is_empty());
    server.stop().await;
    upstream.stop().await;
    std::fs::write(
        root.join("completed.json"),
        serde_json::to_vec_pretty(&json!({"boots":boots,
        "upstreamCalls":2,"keyCleared":true,"provider":"vertex-express-api","credential":"ui-key"}))
        .unwrap(),
    )
    .unwrap();
}
