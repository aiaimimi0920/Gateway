//! Synthetic preset-to-credential compilation: no files, live accounts or network.
use super::*;

fn cases() -> [(&'static str, &'static str); 9] {
    [
        ("accio", "accio"),
        ("kiro", "kiro"),
        ("freebuff", "freebuff"),
        ("grok", "grok_web"),
        ("qwen-web-chat", "qwen_web"),
        ("chataibot", "chataibot_web"),
        ("lumalabs", "lumalabs_web"),
        ("suno", "suno_web"),
        ("udio", "udio_web"),
    ]
}

fn document(preset: &str, special: &str, credentials: serde_json::Value) -> RouteConfigYaml {
    serde_json::from_value(json!({"providers": [{
        "id": "renamed-product", "preset": preset, "base_url": "https://fixture.invalid",
        "explicit_credentials_only": true, "credentials": credentials,
        "supported_models": ["fixture-model"],
        "call_flow": {"call_adapter": "auto", "fingerprint_adapter": "session", "special_adapter": special}
    }]})).unwrap()
}

#[test]
fn call_flow_product_presets_compile_without_creating_or_merging_accounts() {
    for (preset, special) in cases() {
        let empty = document(preset, special, json!([]));
        let p = compile_provider(empty.providers[0].clone()).unwrap();
        assert!(p.credential_pool.is_empty());
        let filled = document(
            preset,
            special,
            json!([
                {"id": "first", "api_key": "synthetic-first"},
                {"id": "second", "api_key": "synthetic-second"}
            ]),
        );
        let p = compile_provider(filled.providers[0].clone()).unwrap();
        assert_eq!(p.id, "renamed-product");
        assert_eq!(p.credential_pool.len(), 2);
        for (index, (id, key)) in [("first", "synthetic-first"), ("second", "synthetic-second")]
            .into_iter()
            .enumerate()
        {
            let credential = &p.credential_pool[index];
            assert_eq!(credential.id, id);
            assert_eq!(credential.payload.api_key, key);
            assert_eq!(credential.payload.base_url, "https://fixture.invalid");
            let flow = credential.payload.call_flow.unwrap();
            assert_eq!(
                flow.special_adapter.adapter(),
                Some(credential.payload.adapter.as_str())
            );
            assert!(credential.payload.request_client_profile.is_none());
        }
    }
}

#[test]
fn call_flow_product_mismatches_fail_before_configuration_is_published() {
    for (preset, special) in cases() {
        for (other, other_special) in cases() {
            if preset != other {
                let cfg = document(preset, other_special, json!([]));
                assert!(compile_provider(cfg.providers[0].clone()).is_err());
            }
        }
        let mut cfg = document(preset, special, json!([]));
        cfg.providers[0]
            .call_flow
            .as_mut()
            .unwrap()
            .fingerprint_adapter = crate::protocol::call_flow::FingerprintAdapter::CodexCli;
        assert!(compile_provider(cfg.providers[0].clone()).is_err());
    }
}
