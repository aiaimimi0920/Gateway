//! Opt-in normal-main UI + live public caller. Controls only trigger callers and observe facts.
use super::{purchase_cycle::*, *};
use std::path::{Path, PathBuf};
type Caller = tokio::task::JoinHandle<(u16, Value)>;
const ACTIONS: &[&str] = &[
    "begin_cancel",
    "cancelled",
    "begin_first",
    "completed_first",
    "end_cancelled",
    "ended_first",
    "begin_second",
    "completed_second",
    "ended_second",
    "finish",
];
#[tokio::test]
#[ignore = "Requires an explicit new synthetic root and dedicated normal-main browser"]
async fn freebuff_public_chat_normal_ui_purchase_cycle_fixture() {
    tokio::time::timeout(Duration::from_secs(1800), Box::pin(serve()))
        .await
        .unwrap();
}
pub(super) fn write(root: &Path, name: &str, value: Value) {
    let stage = root.join(format!("{name}.next"));
    std::fs::write(&stage, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    std::fs::rename(stage, root.join(name)).unwrap();
}
pub(super) async fn ready(host: &Host, sequence: usize, phase: &str) {
    let (active, history) = counts(host).await;
    write(
        &host.root,
        "ready.json",
        json!({"origin":host.server.origin,"pid":std::process::id(),
        "sequence":sequence,"phase":phase,"storageMode":"local","providerId":"pool","credentialId":"account",
        "revision":host.state.route_config.snapshot().revision().id(),"balance":host.balance().await,
        "active":active,"history":history,"pending":pending(host)}),
    );
}
pub(super) async fn command(root: &Path, previous: usize, actions: &[&str]) -> String {
    loop {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let path = root.join("command.json");
        let size = match std::fs::metadata(&path) {
            Ok(value) => value.len(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => panic!("Fixture command metadata failed: {error}"),
        };
        assert!(size <= 65536);
        let bytes = std::fs::read(path).unwrap();
        assert!(bytes.len() <= 65536);
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        let sequence = value["sequence"].as_u64().unwrap() as usize;
        if sequence <= previous {
            continue;
        }
        assert_eq!(sequence, previous + 1);
        let action = value["action"].as_str().unwrap();
        assert_eq!(
            action, actions[previous],
            "Fixture actions cannot be replayed or reordered"
        );
        return action.to_owned();
    }
}
async fn completed(task: &mut Option<Caller>) {
    let reply = tokio::time::timeout(Duration::from_secs(10), task.take().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reply, (200, peer::completion()));
}
async fn serve() {
    let root = PathBuf::from(
        std::env::var_os("GATEWAY_FREEBUFF_PUBLIC_UI_ROOT").expect("Explicit new root required"),
    );
    assert!(root.is_absolute() && root.starts_with(std::env::temp_dir()));
    assert!(root
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("gateway-local-"));
    let peer = peer::Peer::new().await;
    let host = Box::pin(Host::new_at(&peer, root.clone(), Duration::from_secs(180))).await;
    host.seed_device(&peer).await;
    let original = serde_json::to_value(host.state.route_config.snapshot().document()).unwrap();
    let revision = host
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_owned();
    let mut task = None;
    let mut expected = Vec::new();
    let mut ids = Vec::new();
    let mut checkpoints = Vec::new();
    let mut sequence = 0;
    ready(&host, sequence, "initial").await;
    loop {
        let action = command(&root, sequence, ACTIONS).await;
        match action.as_str() {
            "begin_cancel" | "begin_first" | "begin_second" => {
                assert!(task.is_none());
                task = Some(host.public(request()));
                let row = wait_pending(&host, task.as_ref().unwrap()).await;
                assert_eq!(row["quote"]["walletSpend"], 2.0);
                wire(&peer, ids.len(), ids.len());
            }
            "cancelled" => {
                let (status, body) =
                    tokio::time::timeout(Duration::from_secs(10), task.take().unwrap())
                        .await
                        .unwrap()
                        .unwrap();
                assert_eq!(status, 409, "{body}");
                assert_eq!(body["error"]["code"], "freebuff_purchase_cancelled");
                expected.push(("failed", 0));
                audits(&host, &expected, 10).await;
                assert_eq!(counts(&host).await, (0, 0));
                wire(&peer, 0, 0);
            }
            "completed_first" => {
                completed(&mut task).await;
                ids.push(attempt(&peer, 0));
                expected.push(("completed", 1));
                audits(&host, &expected, 9).await;
                assert_eq!(counts(&host).await, (1, 0));
                wire(&peer, 1, 0);
                let (status, body) = host.public(request()).await.unwrap();
                assert_eq!(status, 409, "{body}");
                assert_eq!(body["error"]["code"], "freebuff_public_existing_claim");
                expected.push(("failed", 0));
                audits(&host, &expected, 9).await;
                wire(&peer, 1, 0);
            }
            "end_cancelled" => {
                assert_eq!(counts(&host).await, (1, 0));
                wire(&peer, 1, 0);
            }
            "ended_first" => {
                assert_eq!(counts(&host).await, (0, 1));
                wire(&peer, 1, 1);
            }
            "completed_second" => {
                completed(&mut task).await;
                ids.push(attempt(&peer, 1));
                assert_ne!(ids[0], ids[1]);
                expected.push(("completed", 1));
                audits(&host, &expected, 8).await;
                assert_eq!(counts(&host).await, (1, 1));
                wire(&peer, 2, 1);
            }
            "ended_second" => {
                assert_eq!(counts(&host).await, (0, 2));
                wire(&peer, 2, 2);
            }
            "finish" => break,
            _ => unreachable!(),
        }
        assert_eq!(
            serde_json::to_value(host.state.route_config.snapshot().document()).unwrap(),
            original
        );
        assert_eq!(host.state.route_config.snapshot().revision().id(), revision);
        sequence += 1;
        ready(&host, sequence, &action).await;
        checkpoints.push(
            serde_json::from_slice::<Value>(&std::fs::read(root.join("ready.json")).unwrap())
                .unwrap(),
        );
    }
    assert!(task.is_none());
    assert_eq!(sequence, ACTIONS.len() - 1);
    assert_eq!(counts(&host).await, (0, 2));
    audits(&host, &expected, 8).await;
    wire(&peer, 2, 2);
    host.stop().await;
    peer.server.close().await;
    write(
        &root,
        "completed.json",
        json!({"sequence":sequence + 1,"attemptIds":ids,
        "active":0,"history":2,"balance":8,"admissions":2,"modelRequests":2,"finishes":2,"signedDeletes":2,
        "checkpoints":checkpoints,"sameDocument":true,"sameRevision":true,"joined":true,
        "scope":"Normal main browser + live public caller + real Local TCP/SQLite; synthetic upstream, not a native package or real supplier"}),
    );
}
