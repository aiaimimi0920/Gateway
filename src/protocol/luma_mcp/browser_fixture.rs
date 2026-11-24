//! Synthetic saved accounts, read-only replies and exact wire assertions for the normal UI host.
use super::{fixture::Reply, VERSION};
use crate::state::AppState;
use serde_json::{json, Value};
use std::{path::Path, sync::Arc, time::Duration};
#[path = "../../../tests/fixtures/luma_mcp.rs"]
mod saved;
#[path = "../../../tests/console_contract_support/mod.rs"]
mod support;

pub(super) const SESSIONS: [&str; 6] = [
    "tools-1", "models-2", "image-3", "video-4", "empty-5", "error-6",
];
pub(super) async fn state(root: &Path) -> Arc<AppState> {
    let mut config = support::test_config();
    config.storage_mode = crate::config::GatewayStorageMode::Local;
    config.gateway_api_key = None;
    config.gateway_api_key_secret = None;
    config.console.management_ui_enabled = false;
    config.console.state_dir = root.join("state");
    config.console.routes_file = root.join("routes.yaml");
    write_routes(&config.console.routes_file);
    let state = Box::pin(crate::runtime::build_app_state(config))
        .await
        .unwrap();
    assert!(state.pg_pool.is_none() && state.redis_pool.is_closed());
    state
}
pub(super) fn write_routes(path: &Path) {
    let mut provider = saved::provider();
    let mut other = provider["credentials"][0].clone();
    other["id"] = json!("other-mcp");
    other["api_key"] = json!("synthetic-other");
    provider["credentials"].as_array_mut().unwrap().push(other);
    std::fs::write(
        path,
        serde_yaml::to_string(&saved::document(provider)).unwrap(),
    )
    .unwrap();
}
pub(super) fn replies() -> Vec<Reply> {
    let tool = json!({"name":"luma_model_list","title":"合成模型列表",
        "description":"<script>window.catalogInjected=true</script>","inputSchema":{"type":"object","properties":{}},
        "outputSchema":{"type":"object","properties":{"models":{"type":"array"}}},
        "annotations":{"readOnlyHint":true,"idempotentHint":true},"_meta":{"private":"excluded-tool-metadata"}});
    let models = ["image", "video", "audio"].map(|kind| json!({
        "id":format!("synthetic-{kind}"),"modality":kind,"aliases":[],"actions":[],"capabilities":{},"pricing":null,"strengths":null}));
    let describe = |kind: &str| {
        json!({"model":format!("synthetic-{kind}"),"type":kind,
        "parameters":{"type":"object","properties":{"quality":{"enum":["standard"]}}},"media_roles":[],"guidance":null})
    };
    let payloads = [
        json!({"tools":[tool]}),
        json!({"structuredContent":{"source":"registry","models":models},"isError":false}),
        json!({"structuredContent":describe("image"),"isError":false}),
        json!({"content":[{"type":"text","text":describe("video").to_string()}]}),
        json!({"structuredContent":{"source":"registry","models":[]},"isError":false}),
        json!({"private":"synthetic-private-upstream-error"}),
    ];
    let mut replies = Vec::new();
    for (session, payload) in SESSIONS.into_iter().zip(payloads) {
        let mut init = Reply::json(
            json!({"jsonrpc":"2.0","id":1,"result":{"protocolVersion":VERSION,"capabilities":{"tools":{}}}}),
        );
        init.session = Some(session);
        replies.extend([
            init,
            Reply::initialized(),
            Reply::json(json!({"jsonrpc":"2.0","id":2,"result":payload})),
        ]);
    }
    replies
}
pub(super) fn assert_wire(
    calls: &[(axum::http::HeaderMap, Value)],
    deletes: &[axum::http::HeaderMap],
) -> Value {
    assert_eq!(calls.len(), 18);
    assert_eq!(deletes.len(), 6);
    let expected = [
        ("tools/list", json!({})),
        (
            "tools/call",
            json!({"name":"luma_model_list","arguments":{}}),
        ),
        (
            "tools/call",
            json!({"name":"luma_model_describe","arguments":{"model":"synthetic-image","type":"image"}}),
        ),
        (
            "tools/call",
            json!({"name":"luma_model_describe","arguments":{"model":"synthetic-video","type":"video"}}),
        ),
        (
            "tools/call",
            json!({"name":"luma_model_list","arguments":{}}),
        ),
        ("tools/list", json!({})),
    ];
    for (index, ((method, params), session)) in expected.into_iter().zip(SESSIONS).enumerate() {
        let group = &calls[index * 3..index * 3 + 3];
        assert_eq!(group[0].1["method"], "initialize");
        assert_eq!(group[0].1["params"]["protocolVersion"], VERSION);
        assert!(!group[0].0.contains_key("mcp-session-id"));
        assert_eq!(group[1].1["method"], "notifications/initialized");
        assert_eq!(group[2].1["method"], method);
        assert_eq!(group[2].1["params"], params);
        for headers in [&group[1].0, &group[2].0, &deletes[index]] {
            assert_eq!(headers["mcp-session-id"], session);
            assert_eq!(headers["mcp-protocol-version"], VERSION);
        }
    }
    for headers in calls
        .iter()
        .map(|(headers, _)| headers)
        .chain(deletes.iter())
    {
        assert_eq!(headers["authorization"], "Bearer synthetic-access");
        for name in [
            "cookie",
            "x-api-key",
            "x-management-token",
            "x-secret-grant",
        ] {
            assert!(!headers.contains_key(name));
        }
    }
    json!({"posts":calls.len(),"deletes":deletes.len(),"exactAccount":true,"sessionIsolation":true,"fixedReadOnlyTools":true})
}
pub(super) async fn close(state: &Arc<AppState>) {
    crate::runtime::mark_runtime_draining(state, "luma_mcp_browser_finish");
    state.shutdown.request("luma_mcp_browser_finish");
    tokio::time::timeout(Duration::from_secs(3), async {
        while Arc::strong_count(state) > 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    state.local_runtime.as_ref().unwrap().close().await;
    state
        .upstream_client
        .freebuff
        .shutdown(Duration::from_secs(1))
        .await
        .unwrap();
}
