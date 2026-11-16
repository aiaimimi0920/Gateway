//! Fixed-source threshold/extraction vectors and native failure policies, not real model claims.
use super::super::root_context::ContextForecast;
use super::*;

fn preview(reported_tokens: Option<u64>, fixed_tokens: u64, limit: u64) -> RootPreview {
    RootPreview {
        forecast: ContextForecast {
            estimated_tokens: 1,
            tokens: 1,
            limit,
            calibrated: reported_tokens.is_some(),
        },
        reported_tokens,
        fixed_tokens,
    }
}
fn history() -> RootHistory {
    let messages = (0..6)
        .map(|i| json!({"role":if i%2==0 {"user"} else {"assistant"},"content":"past"}))
        .collect();
    let mut history = RootHistory::new(messages, 32768).unwrap();
    history
        .begin_turn(json!({"role":"user","content":"live"}))
        .unwrap();
    history
}
fn completion(text: &str) -> Value {
    json!({"choices":[{"message":{"role":"assistant","content":text},"finish_reason":"stop"}]})
}
#[test]
fn freebuff_root_summary_trigger_uses_matching_report_not_forecast_or_cache_expiry() {
    let h = history();
    for (reported, expected) in [
        (None, false),
        (Some(8000), false),
        (Some(8001), true),
        (Some(10001), true),
    ] {
        let mut p = preview(reported, 100, 10000);
        p.forecast.tokens = 999999;
        assert_eq!(due(p, &h).unwrap(), expected);
    }
    // A fixed prompt too large to leave room skips the convenience threshold, not the hard budget.
    assert!(!due(preview(Some(9000), 7000, 10000), &h).unwrap());
    assert!(due(preview(Some(10001), 7000, 10000), &h).unwrap());
    let mut only_live = RootHistory::new(vec![], 32768).unwrap();
    only_live
        .begin_turn(json!({"role":"user","content":"live"}))
        .unwrap();
    assert!(!due(preview(Some(10001), 100, 10000), &only_live).unwrap());
}
#[test]
fn freebuff_root_summary_request_keeps_prefix_tools_and_opaque_model_and_limits_output() {
    let body = json!({"model":"fbm1.synthetic","messages":[{"role":"system","content":"fixed"},{"role":"user","content":"history"}],"tools":[{"synthetic":true}],"stream":false,"provider":{"data_collection":"deny"}});
    let summary = request(body.clone(), 10000);
    assert_eq!(summary["model"], body["model"]);
    assert_eq!(
        &summary["messages"].as_array().unwrap()[..2],
        body["messages"].as_array().unwrap()
    );
    assert_eq!(summary["tools"], body["tools"]);
    assert_eq!(summary["provider"], body["provider"]);
    assert_eq!(summary["stream"], true);
    assert_eq!(summary["max_tokens"], 16384);
    let prompt = summary["messages"][2]["content"].as_str().unwrap();
    assert!(prompt.starts_with("<system_instructions>Stop working on the task."));
    assert!(prompt.ends_with("</system_instructions>"));
    assert!(prompt.contains("under about 1000 tokens"));
    assert_eq!(summary["codebuff_metadata"]["compaction_summary"], "1");
    assert_eq!(
        summary["codebuff_metadata"]["compaction_summary_id"],
        summary["codebuff_metadata"]["compaction_request_id"]
    );
}
#[test]
fn freebuff_root_summary_extracts_english_translated_and_think_without_preamble() {
    let expected = format!("## Objective\n{}", "context ".repeat(30));
    assert_eq!(
        from_completion(&completion(&format!(
            "preamble<think>do not keep</think>{expected}"
        )))
        .unwrap(),
        expected.trim()
    );
    let translated = (0..6)
        .map(|i| format!("## 章节{i}\n{}\n", "上下文 ".repeat(15)))
        .collect::<String>();
    assert_eq!(
        from_completion(&completion(&format!("preamble\n{translated}"))).unwrap(),
        translated.trim()
    );
    let too_few = (0..5)
        .map(|i| format!("## Part{i}\n{}\n", "x".repeat(100)))
        .collect::<String>();
    assert_eq!(
        from_completion(&completion(&too_few)),
        Err(BusinessError::SummaryInvalid)
    );
    let memory = memory(&expected);
    assert_eq!(memory["role"], "user");
    assert!(memory["content"]
        .as_str()
        .unwrap()
        .contains("<historical_memory>"));
}
#[test]
fn freebuff_root_summary_rejects_empty_truncated_filtered_tool_and_ambiguous_answers() {
    let text = format!("## Objective\n{}", "valid ".repeat(40));
    for reason in ["length", "content_filter", "tool_calls"] {
        let mut body = completion(&text);
        body["choices"][0]["finish_reason"] = json!(reason);
        assert_eq!(
            from_completion(&body),
            Err(if reason == "length" {
                BusinessError::SummaryOutputLimit
            } else {
                BusinessError::SummaryInvalid
            })
        );
    }
    let mut body = completion(&text);
    body["choices"][0]["message"]["tool_calls"] = json!([{"id":"never execute"}]);
    assert_eq!(from_completion(&body), Err(BusinessError::SummaryInvalid));
    let extra = body["choices"][0].clone();
    body["choices"].as_array_mut().unwrap().push(extra);
    assert_eq!(from_completion(&body), Err(BusinessError::SummaryInvalid));
    for text in [
        "",
        "## Objective\nshort",
        "a long non-summary".repeat(100).as_str(),
    ] {
        assert_eq!(
            from_completion(&completion(text)),
            Err(BusinessError::SummaryInvalid)
        );
    }
    assert_eq!(
        from_completion(&completion(&format!(
            "## Objective\n{}",
            "x".repeat(1024 * 1024)
        ))),
        Err(BusinessError::ResponseTooLarge)
    );
}
