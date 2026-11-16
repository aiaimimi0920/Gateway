//! Root/run-local usage anchoring; native identity witnesses prevent cross-rewrite receipt reuse.
use super::*;
use serde_json::json;

fn assistant() -> Value {
    json!({"role":"assistant","content":"answer"})
}
fn request(messages: Vec<Value>, limit: u64) -> RootRequest {
    let mut native = vec![json!({"role":"system","content":"system"})];
    native.extend(messages);
    let indices: Vec<_> = (1..native.len()).collect();
    RootRequest::new(&json!({"messages":native,"tools":[]}), &indices, limit).unwrap()
}
fn history() -> Vec<Value> {
    vec![json!({"role":"user","content":"start"})]
}
fn receipt(input: u64, output: u64) -> Value {
    json!({"choices":[{"message":assistant()}],"usage":{"prompt_tokens":input,
        "completion_tokens":output,"prompt_tokens_details":{"cached_tokens":input}}})
}
fn anchored(input: u64, output: u64) -> ContextState {
    let mut state = ContextState::default();
    state.prepare(request(history(), 400_000)).unwrap();
    state.observe(Some(&receipt(input, output)));
    state
}
fn appended() -> Vec<Value> {
    let mut next = history();
    next.push(assistant());
    next
}

#[test]
fn freebuff_root_context_usage_anchor_includes_output_but_not_cached_twice() {
    let mut state = anchored(100, 20);
    state.prepare(request(appended(), 400_000)).unwrap();
    let forecast = state.forecast().unwrap();
    assert_eq!(forecast.tokens, 120);
    assert!(forecast.calibrated);
    assert_ne!(forecast.estimated_tokens, forecast.tokens);
}

#[test]
fn freebuff_root_context_delta_uses_input_only_clamped_scale() {
    for (input, output, expected_scale) in
        [(1, 0, 0.5), (10_000, 0, 2.5), (20, 10_000, 20.0 / 13.0)]
    {
        let mut state = anchored(input, output);
        let baseline = state.baseline.as_ref().unwrap();
        assert!((baseline.scale - expected_scale).abs() < 0.000001);
        let tokens = baseline.tokens;
        let estimated = baseline.estimated;
        let mut next = appended();
        next.push(json!({"role":"tool","tool_call_id":"native","content":"result"}));
        let built = request(next, 400_000);
        let delta = built.estimated - estimated;
        state.prepare(built).unwrap();
        assert_eq!(
            state.forecast().unwrap().tokens,
            (tokens as f64 + delta as f64 * expected_scale).round() as u64
        );
    }
}

#[test]
fn freebuff_root_context_missing_or_invalid_usage_preserves_valid_baseline() {
    for usage in [
        json!(null),
        json!({}),
        json!({"prompt_tokens":0,"completion_tokens":1}),
        json!({"prompt_tokens":-1,"completion_tokens":1}),
        json!({"prompt_tokens":1.5,"completion_tokens":1}),
        json!({"prompt_tokens":1,"completion_tokens":-1}),
        json!({"prompt_tokens":MAX_SAFE_INTEGER,"completion_tokens":1}),
    ] {
        let mut state = anchored(100, 20);
        state.prepare(request(appended(), 400_000)).unwrap();
        state.observe(Some(
            &json!({"choices":[{"message":assistant()}],"usage":usage}),
        ));
        assert_eq!(state.baseline.as_ref().unwrap().tokens, 120);
        state.prepare(request(appended(), 400_000)).unwrap();
        assert_eq!(state.forecast().unwrap().tokens, 120);
    }
}

#[test]
fn freebuff_root_context_multi_choice_missing_role_or_failed_decode_cannot_anchor() {
    for phase in 0..4 {
        let mut state = anchored(100, 20);
        state.prepare(request(appended(), 400_000)).unwrap();
        let mut value = receipt(200, 20);
        match phase {
            0 => {
                value["choices"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"message":assistant()}));
            }
            1 => {
                value["choices"][0]["message"]
                    .as_object_mut()
                    .unwrap()
                    .remove("role");
            }
            2 => value["choices"][0]["message"]["role"] = json!("user"),
            _ => {}
        }
        state.observe((phase != 3).then_some(&value));
        assert_eq!(state.baseline.as_ref().unwrap().tokens, 120);
        assert!(state.pending.is_none());
    }
}

#[test]
fn freebuff_root_context_history_prefix_and_fixed_prompt_rewrites_invalidate_receipt() {
    for phase in 0..4 {
        let mut state = anchored(100, 20);
        let mut next = request(appended(), 400_000);
        match phase {
            0 => next.history[0][0] ^= 1,
            1 => next.fixed[0] ^= 1,
            2 => {
                next.history.pop();
            }
            _ => {
                next.history[1] =
                    request::fingerprint(&json!({"role":"assistant","content":"answermerged"}));
            }
        }
        let estimated = next.estimated;
        state.prepare(next).unwrap();
        assert_eq!(state.forecast().unwrap().tokens, estimated);
        assert!(!state.forecast().unwrap().calibrated);
        assert!(state.baseline.is_none());
    }
}

#[test]
fn freebuff_root_context_runtime_budget_rejection_has_no_pending_model_receipt() {
    let mut state = anchored(100, 20);
    assert_eq!(
        state.prepare(request(appended(), 119)),
        Err(BusinessError::ContextBudgetExceeded)
    );
    assert!(state.pending.is_none());
    assert_eq!(state.forecast().unwrap().tokens, 120);
    assert_eq!(state.forecast().unwrap().limit, 119);
    state.prepare(request(appended(), 120)).unwrap();
}

#[test]
fn freebuff_root_context_next_usage_is_per_call_not_cumulative_and_reset_is_root_local() {
    let mut state = anchored(100, 20);
    state.prepare(request(appended(), 400_000)).unwrap();
    state.observe(Some(&receipt(200, 30)));
    assert_eq!(state.baseline.as_ref().unwrap().tokens, 230);
    state.begin();
    assert!(state.forecast().is_none());
    assert!(state.baseline.is_some());
    state.reset();
    assert!(state.baseline.is_none());
    assert!(state.pending.is_none());
    assert!(!format!("{state:?}").contains("answer"));
}

#[test]
fn freebuff_root_context_catalog_budget_overrides_opaque_default_not_compiled_identity() {
    use super::super::{
        root_builder, tests, BusinessOrigin, CatalogSession, ClientSurface, DeviceKey,
        DeviceRegistration, ModelCatalog,
    };
    for (digest, maximum, expected) in [
        ("unknown", None, 400_000),
        ("1e303ac563a6f9cc", None, 400_000),
        ("1e303ac563a6f9cc", Some(110_000), 110_000),
    ] {
        let mut session =
            CatalogSession::new(BusinessOrigin::Codebuff, "synthetic", ClientSurface::Cli).unwrap();
        let mut row = tests::body();
        row["rows"][0]["legacyDigests"] = json!([digest]);
        row["rows"][0]["compaction"] =
            json!({"cacheExpiryMs":3600000,"cacheExpiryMinTokens":140000});
        if let Some(maximum) = maximum {
            row["rows"][0]["compaction"]["maxContextLength"] = json!(maximum);
        }
        session.catalog = Some(ModelCatalog::parse(&serde_json::to_vec(&row).unwrap()).unwrap());
        let (private, public) = tests::pair();
        let device = DeviceRegistration::new(
            &session.binding,
            std::sync::Arc::new(DeviceKey::from_pkcs8(&private, &public).unwrap()),
            "synthetic".into(),
        );
        let (_, built) = session
            .business_requests(&device, "open")
            .unwrap()
            .build_root_step(root_builder::tests::input(), 32768)
            .unwrap();
        assert_eq!(built.limit, expected);
        assert_eq!(
            built.history.len(),
            1,
            "generated reminder must not enter bare history witnesses"
        );
    }
}

#[test]
fn freebuff_root_summary_preview_is_readonly_and_only_a_matching_work_receipt_is_reported() {
    let state = anchored(100, 20);
    let matching = request(appended(), 10000);
    let preview = state.preview(&matching);
    assert_eq!(preview.reported_tokens, Some(120));
    assert_eq!(preview.forecast.tokens, 120);
    assert_eq!(preview.fixed_tokens, 3); // "system" / 3 plus the empty tool schema estimate.
    let mut changed = matching;
    changed.history[0][0] ^= 1;
    assert!(state.preview(&changed).reported_tokens.is_none());
    assert!(state.baseline.is_some());
    assert!(state.pending.is_none());
}
