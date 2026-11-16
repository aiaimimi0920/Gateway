//! The native metadata and the durable finalizer must use one validated usage snapshot.
#[path = "pipeline_send_runtime/config.rs"]
mod config;
#[path = "pipeline_send_runtime/fixture.rs"]
mod fixture;
mod support;
#[path = "bedrock_native_outbound_contract/fixture.rs"]
mod wire;

use axum::http::StatusCode;
use futures::StreamExt;
use neuro_gateway::{
    db::RequestAuditFilters,
    pipeline::{stage_finalize, stage_send, PipelineOutput},
    protocol::canonical::ProtocolFamily,
};
use serde_json::{json, Value};
use tokio::time::timeout;

async fn verify_usage_async(reply: &'static str, expected: Value) {
    let upstream = fixture::Upstream::start(StatusCode::OK, true, reply).await;
    let fixture = fixture::TestState::with_local_limits().await;
    let selected = fixture::candidate(&upstream, "native-bedrock-usage");
    let mut ctx = fixture::context(true, vec![selected]);
    ctx.canonical_req.protocol_family = ProtocolFamily::BedrockConverse;
    ctx.session = Some(neuro_gateway::auth::session::AuthenticatedSession {
        project_id: "fixture-project".into(),
        tenant_id: "fixture-tenant".into(),
        user_id: None,
        credential_ref: None,
        scopes: vec![],
        api_key_id: Some("fixture-key".into()),
        user_credential_id: None,
        access_key_id: None,
        access_key_kind: None,
    });
    stage_finalize::begin_request_audit(&mut ctx, &fixture.state).await;
    let audit_id = ctx.request_audit_id.as_ref().unwrap().clone();
    let PipelineOutput::Sse(stream) =
        timeout(fixture::DEADLINE, stage_send::run(&mut ctx, &fixture.state))
            .await
            .unwrap()
            .unwrap()
    else {
        panic!("stream")
    };
    let frames: Vec<_> = timeout(fixture::DEADLINE, stream.collect()).await.unwrap();
    let metadata = wire::decode(frames.last().unwrap().as_ref().unwrap()).1;
    assert_eq!(metadata["usage"], expected);
    let local = fixture.state.local_runtime.as_ref().unwrap();
    let row = timeout(fixture::DEADLINE, async {
        loop {
            let rows = local
                .list_audits(&RequestAuditFilters::default())
                .await
                .unwrap();
            if let Some(row) = rows
                .into_iter()
                .find(|row| row.id == audit_id && row.status != "running")
            {
                break row;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    upstream.finish().await;
    // Finish before assertions to close SQLite workers even on a count mismatch.
    fixture.finish().await;
    assert_eq!(row.status, "completed");
    assert_eq!(
        row.prompt_tokens.map(i64::from),
        expected["inputTokens"].as_i64()
    );
    assert_eq!(
        row.completion_tokens.map(i64::from),
        expected["outputTokens"].as_i64()
    );
    assert_eq!(
        row.total_tokens.map(i64::from),
        expected["totalTokens"].as_i64()
    );
    assert_eq!(
        row.cache_read_input_tokens.map(i64::from),
        expected["cacheReadInputTokens"].as_i64()
    );
    assert_eq!(
        row.cache_creation_input_tokens.map(i64::from),
        expected["cacheWriteInputTokens"].as_i64()
    );
}

fn verify_usage(reply: &'static str, expected: Value) {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    timeout(fixture::DEADLINE, verify_usage_async(reply, expected))
                        .await
                        .unwrap();
                });
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn native_sparse_usage_and_cache_counts_reach_the_real_finalizer_unchanged() {
    verify_usage(concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"OK\"}}],\"usage\":{\"prompt_tokens\":5}}\n\n",
        "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"completion_tokens\":3}}\n\n",
        "data: {\"choices\":[],\"usage\":{\"total_tokens\":8,\"cache_read_input_tokens\":2,\"cache_creation_input_tokens\":1}}\n\n",
        "data: [DONE]\n\n"), json!({"inputTokens":5,"outputTokens":3,"totalTokens":8,
            "cacheReadInputTokens":2,"cacheWriteInputTokens":1}));
}

#[test]
fn native_explicit_total_is_not_replaced_with_a_different_locally_derived_value() {
    verify_usage(concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"OK\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":2,\"completion_tokens\":3,\"total_tokens\":9}}\n\n",
        "data: [DONE]\n\n"), json!({"inputTokens":2,"outputTokens":3,"totalTokens":9}));
}

#[test]
fn native_explicit_zero_replaces_provisional_nonzero_in_both_output_and_settlement() {
    verify_usage(concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"OK\"}}],\"usage\":{\"prompt_tokens\":2,\"completion_tokens\":3,\"total_tokens\":5}}\n\n",
        "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":0,\"completion_tokens\":0,\"total_tokens\":0}}\n\n",
        "data: [DONE]\n\n"), json!({"inputTokens":0,"outputTokens":0,"totalTokens":0}));
}
