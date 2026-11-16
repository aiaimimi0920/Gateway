//! Opt-in normal-main browser host. Controls assert durable facts; they never fake management replies.
use super::{tcp::*, *};
use std::{path::Path, time::Duration};
#[tokio::test]
#[ignore = "Requires an explicit new fixture root and dedicated browser"]
async fn freebuff_claim_management_local_ui_fixture() {
    tokio::time::timeout(Duration::from_secs(1200), Box::pin(serve()))
        .await
        .unwrap();
}
fn write(root: &Path, name: &str, value: Value) {
    let stage = root.join(format!("{name}.next"));
    std::fs::write(&stage, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    std::fs::rename(stage, root.join(name)).unwrap();
}
fn ready(root: &Path, server: &Host, id: &str, generation: u64, sequence: u64) {
    write(
        root,
        "ready.json",
        json!({"origin":server.origin,"port":server.port,"pid":std::process::id(),"revision":server.revision(),
        "attemptId":id,"providerId":"pool","credentialId":"b","generation":generation,"sequence":sequence,"storageMode":"local"}),
    );
}
async fn serve() {
    let root = PathBuf::from(
        std::env::var_os("GATEWAY_FREEBUFF_CLAIM_UI_ROOT").expect("Explicit new root required"),
    );
    let config = config(&root);
    let peer = wire::Peer::session(json!({"status":"ended","freebucksRefund":0}), 200, None).await;
    let mut server = Host::start(config.clone(), &peer, None).await;
    let id = server.seed_original_claim(&peer).await;
    let original = serde_json::to_value(server.state.route_config.snapshot().document()).unwrap();
    let (mut generation, mut sequence) = (0, 0);
    let mut checkpoints = Vec::new();
    ready(&root, &server, &id, generation, sequence);
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
        assert_eq!(next, sequence + 1);
        sequence = next;
        match command["action"].as_str().unwrap() {
            "after_cancel" => {
                assert_eq!(server.counts().await, (1, 0));
                assert!(peer.hits.lock().unwrap().is_empty());
                checkpoints.push(
                    json!({"stage":"after_cancel","active":1,"history":0,"upstreamRequests":0}),
                );
            }
            "after_end" => {
                assert_eq!(server.counts().await, (0, 1));
                assert_end_wire(&peer, &id);
                checkpoints.push(json!({"stage":"after_end","active":0,"history":1,"upstreamRequests":2,"deletes":1}));
            }
            "restart" => {
                assert_eq!(generation, 0);
                assert_eq!(server.counts().await, (0, 1));
                assert_end_wire(&peer, &id);
                let old_grant = server.grant().await;
                let revision = server.revision();
                let port = server.port;
                server.stop().await;
                server = Host::start(config.clone(), &peer, Some(port)).await;
                generation += 1;
                assert_eq!(server.revision(), revision);
                assert_eq!(server.counts().await, (0, 1));
                assert_eq!(
                    serde_json::to_value(server.state.route_config.snapshot().document()).unwrap(),
                    original
                );
                assert_eq!(
                    server
                        .request(ENDPOINT, server.input(), Some(&old_grant))
                        .await
                        .0,
                    403
                );
                checkpoints.push(json!({"stage":"restart","active":0,"history":1,"samePort":true,"sameRoot":true,"oldGrantRejected":true}));
            }
            "finish" => break,
            action => panic!("Unknown fixture command: {action}"),
        }
        ready(&root, &server, &id, generation, sequence);
    }
    assert_eq!(generation, 1);
    assert_eq!(server.counts().await, (0, 1));
    assert_end_wire(&peer, &id);
    assert_eq!(checkpoints.len(), 3);
    assert_eq!(checkpoints[0]["stage"], "after_cancel");
    assert_eq!(checkpoints[1]["stage"], "after_end");
    assert_eq!(checkpoints[2]["stage"], "restart");
    server.stop().await;
    peer.close().await;
    write(
        &root,
        "completed.json",
        json!({"attemptId":id,"credentialId":"b","active":0,"history":1,"finalRefund":0,
        "checkpoints":checkpoints,"signedDeletes":1,"admissionPosts":0,"joined":true,
        "scope":"Normal main browser and real management TCP/Local runtime; synthetic existing claim, not a real purchase or native package"}),
    );
}
