//! Direct product entrypoints reject incompatible policy before session or lease work.
use super::*;

#[tokio::test]
async fn call_flow_freebuff_direct_entries_fail_before_network_or_runtime_mutation() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let payload: ProviderAccountPayload = serde_json::from_value(serde_json::json!({
        "adapter":"freebuff_compatible", "base_url":format!("http://{}", listener.local_addr().unwrap()),
        "api_key":"fixture", "call_flow":{"call_adapter":"anthropic","special_adapter":"none"}
    })).unwrap();
    let req = crate::protocol::responses::normalize_responses(
        serde_json::json!({"model":"fixture","input":"hello"}),
    )
    .unwrap();
    let runtime = Arc::new(RunRuntime::default());
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(1))
        .build()
        .unwrap();
    for lane in 0..3 {
        let error = match lane {
            0 => execute(&runtime, &client, &payload, &req, "fixture", None)
                .await
                .err(),
            1 => execute_stream(&runtime, &client, &payload, &req, "fixture", None)
                .await
                .err(),
            _ => probe_payload(&runtime, &client, &payload).await.err(),
        }
        .expect("invalid flow must fail");
        assert!(
            error.message.starts_with("Invalid call_flow:"),
            "lane={lane}: {error}"
        );
    }
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(30), listener.accept())
            .await
            .is_err()
    );
}
