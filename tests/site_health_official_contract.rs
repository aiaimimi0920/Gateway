//! Official preset configuration must not redirect or forward monitoring credentials.
use neuro_gateway::site_health::{allows, ProbePolicy, SiteSnapshot};
use serde_json::json;

const OFFICIAL_PRESETS: &[(&str, &str)] = &[
    ("openai-status", "https://status.openai.com/"),
    ("anthropic-status", "https://status.claude.com/"),
    ("deepseek-status", "https://status.deepseek.com/"),
    ("nvidia-status", "https://status.ngc.nvidia.com/"),
    ("gemini-status", "https://aistudio.google.com/status"),
    ("openrouter-status", "https://status.openrouter.ai/"),
    ("cohere-status", "https://status.cohere.com/"),
    ("groq-status", "https://groqstatus.com/"),
    ("poe-status", "https://status.poe.com/"),
    ("perplexity-status", "https://status.perplexity.com/"),
    ("linkup-status", "https://status.linkup.so/"),
    ("exa-status", "https://status.exa.ai/"),
    ("jina-status", "https://status.jina.ai/"),
    ("together-status", "https://status.together.ai/"),
    ("mistral-status", "https://status.mistral.ai/"),
    ("xai-status", "https://status.x.ai/"),
    ("tavily-status", "https://status.tavily.com/"),
    ("you-status", "https://status.you.com/"),
    (
        "azure-status",
        "https://azure.status.microsoft/en-us/status",
    ),
    ("qwen-status", "https://status.aliyun.com/"),
];

#[test]
fn official_presets_are_public_and_origin_locked() {
    for &(adapter, url) in OFFICIAL_PRESETS {
        let mut policy: ProbePolicy = serde_json::from_value(json!({"adapter":adapter})).unwrap();
        assert!(policy.validate("https://provider.example/v1").is_ok());
        policy.site_url = Some(url.into());
        assert!(policy.validate("https://provider.example/v1").is_ok());
        policy.site_url = Some("https://other.example/".into());
        assert!(policy.validate("https://provider.example/v1").is_err());
        policy.site_url = Some(url.into());
        policy.auth.token = Some("must-not-leak".into());
        assert!(policy.validate("https://provider.example/v1").is_err());
    }
}

#[test]
fn codex_exception_is_categorical_scoped_and_fenced() {
    let mut policy: ProbePolicy = serde_json::from_value(
        json!({"adapter":"openai-status", "mode":"restrict", "unknown_policy":"block"}),
    )
    .unwrap();
    let mut snapshot: SiteSnapshot = serde_json::from_value(json!({"provider_id":"p", "checked_at":1000,
        "binding":"b", "selected_template":"openai-status", "models":{},
        "probes":[{"template":"openai-status", "endpoint":"https://status.openai.com/api/v2/components.json",
        "status":"supported", "http_status":200, "model_count":0, "models":{},
        "observations":[{"name":"Codex API", "status":"operational", "model":null}]}]})).unwrap();
    for model in ["gpt-6-astra", "arbitrary-provider-model"] {
        assert!(allows(&policy, Some(&snapshot), "b", model, 1001));
    }
    for status in [
        "degraded_performance",
        "partial_outage",
        "major_outage",
        "unknown",
        "new_state",
    ] {
        snapshot.probes[0].observations[0].status = status.into();
        assert!(!allows(&policy, Some(&snapshot), "b", "m", 1001));
    }
    snapshot.probes[0].observations[0].status = "operational".into();
    for (binding, now) in [("changed", 1001), ("b", 999), ("b", 1900)] {
        assert!(!allows(&policy, Some(&snapshot), binding, "m", now));
    }
    snapshot.probes[0].observations[0].name = "Responses".into();
    assert!(!allows(&policy, Some(&snapshot), "b", "m", 1001));
    policy.adapter = serde_json::from_value(json!("anthropic-status")).unwrap();
    assert!(!allows(&policy, Some(&snapshot), "b", "m", 1001));
}

#[tokio::test]
#[ignore = "Explicit opt-in public status fetch; no model calls or user credentials"]
async fn official_public_status_smoke() {
    let mut failures = Vec::new();
    for &(adapter, _) in OFFICIAL_PRESETS {
        let policy = serde_json::from_value(json!({"adapter":adapter})).unwrap();
        let result = neuro_gateway::site_health::discovery::discover(
            "fixture",
            "https://provider.example",
            &policy,
        )
        .await;
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                failures.push(format!("{adapter}: {error}"));
                continue;
            }
        };
        println!("{adapter}: {}", serde_json::to_string(&result).unwrap());
        if result.selected_template.as_deref() != Some(adapter) || !result.models.is_empty() {
            failures.push(format!(
                "{adapter}: unsupported schema or fabricated percentages"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn additional_component_adapters_do_not_inherit_the_codex_exception() {
    for &(adapter, url) in OFFICIAL_PRESETS.iter().skip(6) {
        let policy: ProbePolicy = serde_json::from_value(json!({
            "adapter":adapter, "mode":"restrict", "unknown_policy":"block"
        }))
        .unwrap();
        let snapshot: SiteSnapshot = serde_json::from_value(json!({
            "provider_id":"p", "checked_at":1000, "binding":"b", "selected_template":adapter,
            "models":{}, "probes":[{"template":adapter,"endpoint":url,"status":"supported",
            "http_status":200,"model_count":0,"models":{},
            "observations":[{"name":"Codex API","status":"operational","model":null}]}]
        }))
        .unwrap();
        assert!(
            !allows(&policy, Some(&snapshot), "b", "any-model", 1001),
            "{adapter}"
        );
    }
}
