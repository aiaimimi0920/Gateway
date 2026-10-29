use super::*;
use crate::provider_identity::ProviderAliases;

fn context() -> PipelineContext {
    PipelineContext::new(
        crate::protocol::canonical::CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: Some("test-model".into()),
            stream: false,
            messages: vec![],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: serde_json::json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: Default::default(),
        },
        None,
    )
}

#[test]
fn local_wait_reserves_identity_growth_without_changing_remote_accounting() {
    let mut ctx = context();
    let remote = retained_bytes(&ctx, false).unwrap();
    let empty = retained_bytes(&ctx, true).unwrap();
    assert_eq!(empty - remote, ProviderAliases::WAIT_SNAPSHOT_BYTES);
    ctx.provider_identities = ProviderAliases::new(
        (0..256)
            .map(|i| (format!("old-{i:0252}"), format!("new-{i:0252}")))
            .collect(),
    )
    .unwrap();
    assert_eq!(retained_bytes(&ctx, true).unwrap(), empty);
}

#[test]
fn identity_reservation_counts_against_queue_limit_and_is_released() {
    let queue = Arc::new(crate::request_wait::WaitRegistry::default());
    let ctx = context();
    let local_bytes = retained_bytes(&ctx, true).unwrap();
    let other = queue
        .enter(
            "other".into(),
            "other".into(),
            64 * 1024 * 1024 - local_bytes,
        )
        .unwrap();
    let local = queue
        .enter("key".into(), "old".into(), local_bytes)
        .unwrap();
    assert_eq!(
        queue
            .enter("next".into(), "new".into(), 1)
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("recovery_wait_queue_full")
    );
    drop(local);
    let replacement = queue
        .enter("key".into(), "new".into(), local_bytes)
        .unwrap();
    drop((other, replacement));
    assert!(queue
        .enter("all".into(), "p".into(), 64 * 1024 * 1024)
        .is_ok());
}
