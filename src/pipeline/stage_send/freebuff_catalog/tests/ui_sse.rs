//! Opt-in normal main + all three live SSE callers; controls cannot make management decisions.
use super::{purchase_cycle::*, *};
use std::path::PathBuf;
mod caller;
mod cycle;
const ACTIONS: &[&str] = &[
    "begin_cancel",
    "cancelled",
    "begin_chat",
    "live_chat",
    "eof_chat",
    "completed_chat",
    "end_cancelled",
    "ended_chat",
    "begin_responses",
    "live_responses",
    "eof_responses",
    "completed_responses",
    "ended_responses",
    "begin_messages",
    "live_messages",
    "eof_messages",
    "completed_messages",
    "ended_messages",
    "finish",
];
#[tokio::test]
#[ignore = "Requires an explicit new synthetic root and dedicated normal-main browser"]
async fn freebuff_public_sse_normal_ui_three_protocol_purchase_cycle_fixture() {
    tokio::time::timeout(Duration::from_secs(1800), Box::pin(serve()))
        .await
        .unwrap();
}
async fn serve() {
    let root = PathBuf::from(
        std::env::var_os("GATEWAY_FREEBUFF_PUBLIC_SSE_UI_ROOT")
            .expect("Explicit new root required"),
    );
    assert!(root.is_absolute() && root.starts_with(std::env::temp_dir()));
    assert!(root
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("gateway-local-"));
    let mut cycle = cycle::Cycle::new(root).await;
    let mut checkpoints = Vec::new();
    ui::ready(&cycle.host, 0, "initial").await;
    for (sequence, expected) in ACTIONS.iter().enumerate() {
        let action = ui::command(&cycle.host.root, sequence, ACTIONS).await;
        assert_eq!(&action, expected);
        if action == "finish" {
            cycle.close(sequence + 1, checkpoints).await;
            return;
        }
        cycle.apply(&action).await;
        cycle.check_document();
        ui::ready(&cycle.host, sequence + 1, &action).await;
        checkpoints.push(
            serde_json::from_slice::<Value>(
                &std::fs::read(cycle.host.root.join("ready.json")).unwrap(),
            )
            .unwrap(),
        );
    }
    unreachable!();
}
