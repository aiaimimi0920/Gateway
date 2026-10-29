//! Official product ownership must not bypass common plan/transport contracts.
use super::*;
use serde_json::json;

#[test]
fn call_flow_official_context_retains_common_extra_body_merge() {
    let mut payload = make_payload("https://api.openai.com");
    payload.extra_body = Some(
        serde_json::from_value(
            json!({"model":"must-not-win", "stream":true,"fixture_extension":7}),
        )
        .unwrap(),
    );
    let req = make_request(EndpointKind::Responses);
    let context = prepare_nonstreaming_execute_context(&payload, &req, "fixture", None).unwrap();
    let body = context.plan.body.unwrap();
    assert_eq!(body["model"], "fixture");
    assert_eq!(body["stream"], false);
    assert_eq!(body["fixture_extension"], 7);
}

#[test]
fn call_flow_official_cpa_rejects_retired_transport_for_all_three_sends() {
    const MARKER: &str = "GATEWAY_TEST_OFFICIAL_CPA_CHILD";
    if std::env::var_os(MARKER).is_none() {
        // Environment policy is isolated in a child test process; no process-wide races.
        let module = module_path!().split_once("::").unwrap().1;
        let name = format!(
            "{module}::call_flow_official_cpa_rejects_retired_transport_for_all_three_sends"
        );
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &name, "--nocapture"])
            .env(MARKER, "1")
            .env("GATEWAY_CPA_TRANSPORT", "go")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"),
            "child must execute exactly one test"
        );
        return;
    }
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut payload = make_payload(&format!("http://{}/api.openai.com", listener.local_addr().unwrap()));
        payload.call_flow = Some(serde_json::from_value(json!({"call_adapter":"openai","fingerprint_adapter":"cpa","special_adapter":"none"})).unwrap());
        let req = make_request(EndpointKind::Responses);
        let client = UpstreamClient::new(1);
        for lane in 0..3 {
            let error = match lane {
                0 => client.execute_chatgpt_official_nonstreaming(&payload, &req, "fixture", None).await.err(),
                1 => client.execute_chatgpt_official_streaming(&payload, &req, "fixture", None).await.err(),
                _ => client.execute_chatgpt_official_forced_streaming_accumulate(&payload, &req, "fixture", None).await.err(),
            }.expect("retired CPA transport must fail before sending");
            assert_eq!(error.code.as_deref(), Some("cpa_transport_configuration"), "lane={lane}: {error}");
        }
        assert!(tokio::time::timeout(std::time::Duration::from_millis(30), listener.accept()).await.is_err());
    });
}
