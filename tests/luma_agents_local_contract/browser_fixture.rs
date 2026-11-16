//! Explicit new-directory browser fixture; commands are local files, not production endpoints.
use super::{config, runtime::LocalServer};
use serde_json::json;
use std::{path::Path, time::Duration};

#[tokio::test]
#[ignore = "Requires an explicit new fixture directory and a dedicated browser session"]
async fn luma_agents_local_ui_fixture() {
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
        std::env::var_os("GATEWAY_LUMA_UI_ROOT").expect("Explicit fixture root required"),
    );
    let config = config(&root);
    let mut server = LocalServer::start(config.clone(), None).await;
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
                assert!(generation < 2, "Exactly two deliberate runtime rebuilds");
                let port = server.port;
                let revision = server.revision();
                server.stop().await;
                server = LocalServer::start(config.clone(), Some(port)).await;
                assert_eq!(server.revision(), revision);
                generation += 1;
                server.assert_account("lumalabs-api", "ui-luma", generation == 1);
                let providers = server.state.route_config.get_providers();
                let credential = &providers[0].credential_pool[0];
                // The ordinary UI edits model metadata; it has no account-name field.
                assert_eq!(credential.supported_models, vec!["uni-1", "ray-3.2"]);
                boots.push(json!({"generation":generation,"revision":revision,"keyPresent":generation == 1}));
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
    server.assert_account("lumalabs-api", "ui-luma", false);
    server.stop().await;
    // Success is recorded only after HTTP and the real SQLite owner have closed.
    std::fs::write(
        root.join("completed.json"),
        serde_json::to_vec_pretty(&json!({"boots":boots,
        "publicMediaPipelineExercised":false,"keyCleared":true,"provider":"lumalabs-api","credential":"ui-luma",
        "scope":"management persistence only; no public media pipeline or settlement"}))
        .unwrap(),
    )
    .unwrap();
}
