use super::*;
use crate::protocol::{canonical::CanonicalToolCall, cursor_remote::native_publication::validate};

fn output() -> CanonicalRelayResponse {
    CanonicalRelayResponse {
        model: "cursor-personal".into(),
        text: "prelude 中文🦀".into(),
        bedrock_turn: None,
        vertex_turn: None,
        model_declaration: None,
        sources: vec![],
        usage: None,
        tool_calls: vec![CanonicalToolCall {
            id: Some("call_one".into()),
            call_type: "function".into(),
            name: Some("echo".into()),
            arguments: Some(r#"{"q":"中文🦀"}"#.into()),
            raw: Default::default(),
        }],
        upstream_status: Some(200),
        finish_reason: Some("tool_calls".into()),
    }
}

#[test]
fn cursor_remote_native_publication_requires_whole_tool_terminal() {
    for family in [
        ProtocolFamily::GeminiGenerateContent,
        ProtocolFamily::CohereChat,
    ] {
        let value = response(family, &output()).unwrap();
        let bytes = Bytes::from(value.to_string());
        validate(family, bytes.clone(), false).unwrap();
        assert!(validate(family, bytes.slice(..bytes.len() - 1), false).is_err());
        let events = frames(family, &value);
        let full = Bytes::from(events.concat());
        validate(family, full.clone(), true).unwrap();
        assert!(validate(family, full.slice(..full.len() - 1), true).is_err());
        let doubled = Bytes::from([full.clone(), full.clone()].concat());
        assert!(validate(family, doubled, true).is_err());
        let mut bad = value.clone();
        if family == ProtocolFamily::GeminiGenerateContent {
            bad["candidates"][0]["finishReason"] = json!("MAX_TOKENS");
        } else {
            bad["finish_reason"] = json!("COMPLETE");
        }
        assert!(validate(family, Bytes::from(bad.to_string()), false).is_err());
        assert!(validate(family, Bytes::from(frames(family, &bad).concat()), true).is_err());
        if family == ProtocolFamily::CohereChat {
            for skip in 0..events.len() {
                let missing = Bytes::from(
                    events
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| *i != skip)
                        .flat_map(|(_, bytes)| bytes.iter().copied())
                        .collect::<Vec<_>>(),
                );
                assert!(
                    validate(family, missing, true).is_err(),
                    "missing event {skip}"
                );
            }
        }
    }
}

#[test]
fn cursor_remote_native_output_does_not_invent_usage_model_or_thinking() {
    for family in [
        ProtocolFamily::GeminiGenerateContent,
        ProtocolFamily::CohereChat,
    ] {
        let response = response(family, &output()).unwrap();
        for field in [
            "usage",
            "usageMetadata",
            "modelVersion",
            "model",
            "thinking",
            "tool_plan",
        ] {
            assert!(response.get(field).is_none());
        }
        assert!(!response.to_string().contains("reasoning"));
    }
}
