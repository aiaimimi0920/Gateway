//! Explicit interactive opt-in; no account persistence, state export or content generation.
use super::*;
use crate::upstream::{
    execute_producer_browser_worker_process, PreparedProducerBrowserWorkerLaunch,
};
use serde_json::json;
use std::path::PathBuf;

#[tokio::test]
#[ignore = "requires explicit FLOW_WEB_LIVE_READONLY=1 and manual Google login"]
async fn flow_web_live_readonly_authentication() {
    assert_eq!(std::env::var("FLOW_WEB_LIVE_READONLY").as_deref(), Ok("1"));
    let root = PathBuf::from(
        std::env::var_os("FLOW_WEB_LIVE_TEMP_ROOT").expect("explicit temporary root required"),
    );
    assert!(root.is_absolute());
    let output = execute_producer_browser_worker_process(
        PreparedProducerBrowserWorkerLaunch {
            node_bin: "node".into(),
            script_path: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("scripts/flow-auth-worker.mjs"),
            stdin_json: serde_json::to_vec(&json!({"tempRoot":root})).unwrap(),
        },
        Duration::from_secs(25 * 60),
    )
    .await;
    let mut receipt = json!({"provider":"flow-web","loginMaterialExported":false,
        "generationSubmitted":false,"generationAccepted":false,"runtimeTransport":"direct_http"});
    let result = match output {
        Ok(output) => {
            let client = crate::http_client::builder()
                .redirect(rquest::redirect::Policy::none())
                .build()
                .unwrap();
            read_credits_bundle(&client, &output.stdout)
                .await
                .map(|_| ())
        }
        Err(_) => Err(failure(
            "flow_web_capture_failed",
            "Flow interactive capture failed; private details omitted.",
        )),
    };
    receipt["readOnlyAuthenticationPassed"] = json!(result.is_ok());
    if let Err(error) = &result {
        receipt["errorCode"] = json!(error.code);
    }
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("readonly.safe.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .unwrap();
    assert!(
        result.is_ok(),
        "Flow readonly check failed: {}",
        result.err().and_then(|e| e.code).unwrap_or_default()
    );
}
