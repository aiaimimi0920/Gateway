//! Opt-in normal-main host. File controls rebuild the real runtime; they never fake management replies.
use super::*;

#[tokio::test]
#[ignore = "Requires an explicit new fixture directory and dedicated browser"]
async fn freebuff_local_ui_fixture() {
    tokio::time::timeout(Duration::from_secs(1800), Box::pin(serve()))
        .await
        .unwrap();
}
fn ready(root: &Path, server: &LocalServer, generation: u64, sequence: u64) {
    let value = json!({"origin":server.origin,"port":server.port,"pid":std::process::id(),
        "revision":server.revision(),"generation":generation,"sequence":sequence,"storageMode":"local"});
    let stage = root.join("ready.next.json");
    std::fs::write(&stage, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    std::fs::rename(stage, root.join("ready.json")).unwrap();
}
async fn serve() {
    let root = std::path::PathBuf::from(
        std::env::var_os("GATEWAY_FREEBUFF_UI_ROOT").expect("Explicit new root required"),
    );
    let config = config(&root);
    let peer = Peer::start().await;
    let mut server = LocalServer::start(config.clone(), &peer, None).await;
    let (mut generation, mut sequence) = (0_u64, 0_u64);
    let mut identity = None;
    let mut boots = Vec::new();
    ready(&root, &server, generation, sequence);
    loop {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let bytes = match std::fs::read(root.join("command.json")) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => panic!("Fixture command read failed: {error}"),
        };
        let command: Value = serde_json::from_slice(&bytes).unwrap();
        let next = command["sequence"].as_u64().unwrap();
        if next <= sequence {
            continue;
        }
        sequence = next;
        match command["action"].as_str().unwrap() {
            "restart" => {
                let before = account(&server);
                assert!(before.id.as_deref().is_some_and(|id| !id.is_empty()));
                if let Some(id) = &identity {
                    assert_eq!(before.id.as_ref(), Some(id));
                } else {
                    identity = before.id.clone();
                }
                let devices = peer.devices();
                assert_eq!(
                    peer.counts(),
                    ((generation + 1) as usize, (generation + 1) as usize)
                );
                assert!(devices.iter().all(|device| device == &devices[0]));
                assert_material(&server, (generation + 1) as usize, &devices[0]);
                let revision = server.revision();
                let port = server.port;
                server.stop().await;
                server = LocalServer::start(config.clone(), &peer, Some(port)).await;
                generation += 1;
                assert_eq!(server.revision(), revision);
                assert_eq!(
                    serde_json::to_value(account(&server)).unwrap(),
                    serde_json::to_value(before).unwrap()
                );
                boots.push(
                    json!({"generation":generation,"revision":revision,"credentialId":identity,
                    "samePort":true,"sameRoot":true,"materialPreserved":true}),
                );
            }
            "finish" => break,
            action => panic!("Unknown fixture command: {action}"),
        }
        ready(&root, &server, generation, sequence);
    }
    assert_eq!(generation, 2);
    assert_eq!(account(&server).id, identity);
    assert_eq!(
        account(&server).account_name.as_deref(),
        Some("FreeBuff local UI metadata")
    );
    assert_eq!(peer.counts(), (2, 2));
    assert!(root.join("state/runtime.sqlite3").is_file());
    let (status, public) = server.manage("GET", ROUTE, Value::Null, None).await;
    assert_eq!(status, 200, "{public}");
    assert_public(&public, &peer);
    server.stop().await;
    peer.stop().await;
    std::fs::write(
        root.join("completed.json"),
        serde_json::to_vec_pretty(&json!({"boots":boots,"codePosts":2,"statusGets":2,
        "stableCredential":identity,"sameDevice":true,"publicRedacted":true,"joined":true,
        "scope":"same-process runtime rebuild; no public inference or platform acceptance"}))
        .unwrap(),
    )
    .unwrap();
}
