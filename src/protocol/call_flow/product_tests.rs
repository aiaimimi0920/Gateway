//! Product bindings must not rewrite credentials or invent authentication lifecycles.
use super::*;
use crate::{preset, routing::candidate::ProviderAccountPayload};
use serde_json::{json, Value};

fn products() -> Vec<(&'static str, preset::ProviderPreset)> {
    vec![
        ("accio", preset::accio_preset()),
        ("kiro", preset::kiro_preset()),
        ("freebuff", preset::freebuff_preset()),
        ("grok_web", preset::grok_preset()),
        ("qwen_web", preset::qwen_web_chat_preset()),
        ("chataibot_web", preset::chataibot_preset()),
        ("lumalabs_web", preset::lumalabs_preset()),
        ("suno_web", preset::suno_preset()),
        ("udio_web", preset::udio_preset()),
    ]
}

fn payload(adapter: &str, flow: Value) -> ProviderAccountPayload {
    serde_json::from_value(json!({
        "adapter": adapter, "base_url": "https://fixture.invalid", "api_key": "synthetic-session",
        "headers": {"x-fixture": "unchanged"}, "extra_body": {"fixture": true}, "call_flow": flow
    }))
    .unwrap()
}

#[test]
fn call_flow_products_bind_existing_presets_without_rewriting_materials() {
    for (special, preset) in products() {
        let p = payload(
            &preset.adapter,
            json!({
                "call_adapter": "auto", "fingerprint_adapter": "session", "special_adapter": special
            }),
        );
        let before = serde_json::to_value(&p).unwrap();
        let plan = prepare(&p).unwrap();
        assert_eq!(plan.executor, preset.adapter);
        assert!(plan.executor_owns_fingerprint);
        assert_eq!(serde_json::to_value(plan.payload.as_ref()).unwrap(), before);
        assert_eq!(serde_json::to_value(&p).unwrap(), before);
        let flow: ProviderCallFlow = serde_json::from_value(before["call_flow"].clone()).unwrap();
        assert_eq!(
            flow.special_adapter.adapter(),
            Some(preset.adapter.as_str())
        );
        assert_eq!(
            serde_json::to_value(flow).unwrap()["special_adapter"],
            special
        );
    }
}

#[test]
fn call_flow_products_reject_cross_product_and_standard_api_bindings() {
    for (special, preset) in products() {
        let flow = json!({"special_adapter": special});
        assert!(prepare(&payload("openai_compatible", flow.clone())).is_err());
        for (_, other) in products() {
            if other.adapter != preset.adapter {
                assert!(prepare(&payload(&other.adapter, flow.clone())).is_err());
            }
        }
    }
}

#[test]
fn call_flow_products_reject_api_format_fingerprint_and_executor_disable() {
    for (special, preset) in products() {
        for field in ["call_adapter", "fingerprint_adapter", "special_adapter"] {
            let options: &[&str] = match field {
                "call_adapter" => &["openai", "anthropic", "gemini", "cohere"],
                "fingerprint_adapter" => {
                    &["codex_cli", "claude_cli", "workbuddy", "cpa", "antigravity"]
                }
                _ => &["none"],
            };
            for option in options {
                let mut flow = json!({"special_adapter": special});
                flow[field] = json!(option);
                assert!(prepare(&payload(&preset.adapter, flow)).is_err());
            }
        }
    }
}

#[test]
fn call_flow_products_keep_legacy_and_auto_configuration_unchanged() {
    for (_, preset) in products() {
        for flow in [Value::Null, json!({}), json!({"special_adapter": "auto"})] {
            let p = payload(&preset.adapter, flow);
            let before = serde_json::to_value(&p).unwrap();
            let plan = prepare(&p).unwrap();
            assert_eq!(plan.executor, preset.adapter);
            assert!(plan.executor_owns_fingerprint);
            assert_eq!(serde_json::to_value(plan.payload.as_ref()).unwrap(), before);
        }
    }
}
