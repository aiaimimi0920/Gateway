//! Canonical history has one exclusive writer; summary candidates never mutate their input.
use super::*;
use serde_json::json;

fn old() -> Vec<Value> {
    (0..6)
        .map(|i| json!({"role":if i%2==0 {"user"} else {"assistant"},"content":format!("old-{i}")}))
        .collect()
}
#[test]
fn freebuff_root_summary_history_preserves_current_request_steering_and_native_extensions() {
    let mut history = RootHistory::new(old(), 32768).unwrap();
    assert!(!history.is_live_turn());
    let user = json!({"role":"user","content":[{"type":"text","text":"exact"},{"type":"image_url","image_url":{"url":"synthetic"}}],"host_extension":{"keep":true}});
    let steering = json!({"role":"user","content":"newer instruction"});
    history.begin_turn(user.clone()).unwrap();
    history
        .record_completion(
            &json!({"choices":[{"message":{"role":"assistant","content":"working"}}]}),
        )
        .unwrap();
    history
        .append_tools(vec![
            json!({"role":"tool","tool_call_id":"synthetic","content":"already read"}),
        ])
        .unwrap();
    history.steer(steering.clone()).unwrap();
    let original = history.messages().to_vec();
    let candidate = history
        .compacted(json!({"role":"user","content":"summary"}))
        .unwrap();
    assert_eq!(
        candidate.messages(),
        &[json!({"role":"user","content":"summary"}), user, steering]
    );
    assert!(candidate.is_live_turn());
    assert_eq!(history.messages(), original);
    assert!(!format!("{history:?}").contains("already read"));
}
#[test]
fn freebuff_root_summary_history_failed_edits_are_atomic_and_do_not_reclassify_turns() {
    let mut history = RootHistory::new(old(), 512).unwrap();
    let before = history.messages().to_vec();
    assert_eq!(
        history.begin_turn(json!({"role":"tool"})),
        Err(BusinessError::InvalidInput)
    );
    assert_eq!(
        history.begin_turn(json!({"role":"user","content":"x".repeat(1024)})),
        Err(BusinessError::RequestTooLarge)
    );
    assert_eq!(history.messages(), before);
    assert!(!history.is_live_turn());
    history
        .begin_turn(json!({"role":"user","content":"current"}))
        .unwrap();
    assert_eq!(
        history.append_tools(vec![json!({"role":"assistant"})]),
        Err(BusinessError::InvalidInput)
    );
    let before = history.messages().to_vec();
    assert_eq!(
        history
            .compacted(json!({"role":"user","content":"x".repeat(1024)}))
            .err(),
        Some(BusinessError::RequestTooLarge)
    );
    assert_eq!(history.messages(), before);
}
#[test]
fn freebuff_root_summary_history_new_turn_excludes_previous_live_users_and_noop_is_rejected() {
    let mut history = RootHistory::new(vec![], 32768).unwrap();
    history
        .begin_turn(json!({"role":"user","content":"old request"}))
        .unwrap();
    assert_eq!(
        history
            .compacted(json!({"role":"user","content":"memory"}))
            .err(),
        Some(BusinessError::SummaryInvalid)
    );
    history
        .record_completion(&json!({"choices":[{"message":{"role":"assistant","content":"done"}}]}))
        .unwrap();
    history
        .begin_turn(json!({"role":"user","content":"new request"}))
        .unwrap();
    let candidate = history
        .compacted(json!({"role":"user","content":"memory"}))
        .unwrap();
    assert_eq!(candidate.messages().len(), 2);
    assert_eq!(candidate.messages()[1]["content"], "new request");
    assert!(history.record_completion(&json!({"choices":[]})).is_err());
}
