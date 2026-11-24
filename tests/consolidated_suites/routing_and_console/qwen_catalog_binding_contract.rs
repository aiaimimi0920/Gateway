//! Qwen directory evidence binds to the saved workspace, not just a shared source host.
use neuro_gateway::provider_discovery::model_catalog::{ModelCatalog, OPENAI_SOURCE};
use serde_json::{json, Value};

fn catalog(base: &str, source: &str) -> Value {
    json!({"sourceUrl":source,"accountBaseUrl":base,
        "checkedAt":"2026-11-02T00:00:00Z","models":["qwen-contract-model"]})
}

#[test]
fn all_six_workspace_regions_have_exact_catalog_bindings() {
    for region in [
        "cn-beijing",
        "ap-southeast-1",
        "ap-northeast-1",
        "eu-central-1",
        "us-east-1",
        "cn-hongkong",
    ] {
        let base = format!("https://workspace.{region}.maas.aliyuncs.com/compatible-mode/v1");
        let origin = match region {
            "ap-southeast-1" => "https://dashscope-intl.aliyuncs.com".to_owned(),
            "cn-hongkong" => "https://cn-hongkong.dashscope.aliyuncs.com".to_owned(),
            _ => format!("https://workspace.{region}.maas.aliyuncs.com"),
        };
        let value = catalog(&base, &format!("{origin}/api/v1/models"));
        let parsed: ModelCatalog = serde_json::from_value(value).unwrap();
        parsed.validate(Some("qwen-official-api"), &base).unwrap();
        assert!(parsed.validate(Some("qwen-web-chat"), &base).is_err());
        assert!(parsed
            .validate(Some("qwen-coding-plan-openai"), &base)
            .is_err());
    }
}

#[test]
fn shared_directory_does_not_prove_a_different_workspace_binding() {
    for (base, source) in [
        (
            "https://workspace.ap-southeast-1.maas.aliyuncs.com/compatible-mode/v1",
            "https://dashscope-intl.aliyuncs.com/api/v1/models",
        ),
        (
            "https://workspace.cn-hongkong.maas.aliyuncs.com/compatible-mode/v1",
            "https://cn-hongkong.dashscope.aliyuncs.com/api/v1/models",
        ),
    ] {
        let value = catalog(base, source);
        let parsed: ModelCatalog = serde_json::from_value(value.clone()).unwrap();
        assert!(parsed.validate(Some("qwen-official-api"), base).is_ok());
        let other = base.replace("workspace.", "other.");
        assert!(parsed.validate(Some("qwen-official-api"), &other).is_err());
        for field in ["accountBaseUrl", "sourceUrl"] {
            let mut wrong = value.clone();
            wrong[field] = json!("https://evil.invalid/api/v1/models");
            let parsed: ModelCatalog = serde_json::from_value(wrong).unwrap();
            assert!(parsed.validate(Some("qwen-official-api"), base).is_err());
        }
    }
}

#[test]
fn fixed_catalogs_do_not_accept_the_qwen_binding_field() {
    let value = catalog("https://api.openai.com/v1", OPENAI_SOURCE);
    let parsed: ModelCatalog = serde_json::from_value(value).unwrap();
    assert!(parsed
        .validate(Some("openai"), "https://api.openai.com/v1")
        .is_err());
}

#[test]
fn qwen_requires_binding_but_legacy_fixed_catalogs_remain_byte_compatible() {
    let mut value = catalog(
        "https://workspace.cn-beijing.maas.aliyuncs.com/compatible-mode/v1",
        "https://workspace.cn-beijing.maas.aliyuncs.com/api/v1/models",
    );
    value.as_object_mut().unwrap().remove("accountBaseUrl");
    let qwen: ModelCatalog = serde_json::from_value(value).unwrap();
    assert!(qwen
        .validate(
            Some("qwen-official-api"),
            "https://workspace.cn-beijing.maas.aliyuncs.com/compatible-mode/v1"
        )
        .is_err());
    let old =
        json!({"sourceUrl":OPENAI_SOURCE,"checkedAt":"2026-11-02T00:00:00Z","models":["fixture"]});
    let parsed: ModelCatalog = serde_json::from_value(old.clone()).unwrap();
    parsed
        .validate(Some("openai"), "https://api.openai.com/v1")
        .unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), old);
}
