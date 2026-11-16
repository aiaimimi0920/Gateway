//! Opt-in browser host: explicit disposable root, local backend restart, no production test endpoint.
use super::{assert_wire, config, run, wire, LocalServer};
use serde_json::json;
use std::{path::Path, time::Duration};

#[test]
#[ignore = "Requires an explicit new fixture directory and a dedicated browser session"]
fn kiro_api_key_local_ui_fixture() {
    run(1800, serve_browser);
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
        std::env::var_os("GATEWAY_KIRO_KEY_UI_ROOT").expect("Explicit fixture root required"),
    );
    assert!(
        root.is_absolute() && !root.exists(),
        "Fixture root must be a new absolute path"
    );
    let config = config(&root);
    let upstream = wire::Server::stream(wire::event_frame("local-answer"), false).await;
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
                assert_eq!(providers[0].id, "kiro-api");
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
        "Verify saved Key and clear across separate runtime rebuilds"
    );
    assert_eq!(
        upstream.attempts(),
        2,
        "Only the two explicitly requested synthetic generations may send"
    );
    assert_wire(&upstream);
    let providers = server.state.route_config.get_providers();
    assert_eq!(providers[0].id, "kiro-api");
    assert_eq!(providers[0].credential_pool[0].id, "ui-key");
    assert!(providers[0].credential_pool[0].payload.api_key.is_empty());
    server.stop().await;
    upstream.stop().await;
    std::fs::write(
        root.join("completed.json"),
        serde_json::to_vec_pretty(&json!({"boots":boots,
        "upstreamCalls":2,"keyCleared":true,"provider":"kiro-api","credential":"ui-key"}))
        .unwrap(),
    )
    .unwrap();
}
