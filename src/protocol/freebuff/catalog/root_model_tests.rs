//! Same-snapshot model inputs and fixed compiled identity are not prompt/harness acceptance.
use super::*;
use crate::protocol::freebuff::catalog::{tests::body, ModelCatalog};
use serde_json::{json, Value};

fn catalog(compaction: Option<Value>, digests: &[&str]) -> ModelCatalog {
    let mut value = body();
    if let Some(policy) = compaction {
        value["rows"][0]["compaction"] = policy;
    }
    value["rows"][0]["legacyDigests"] = json!(digests);
    ModelCatalog::parse(&serde_json::to_vec(&value).unwrap()).unwrap()
}
fn policy(lean: bool) -> Value {
    json!({"cacheExpiryMs":3600000,"cacheExpiryMinTokens":0,
        "maxContextLength":110000,"lean":lean})
}

#[test]
fn freebuff_root_model_inputs_keep_same_row_handle_label_policy_and_fetch() {
    let mut first = body();
    first["rows"][0]["compaction"] = policy(false);
    first["rows"][0]["legacyDigests"] = json!(["1e303ac563a6f9cc"]);
    first["rows"][0]["displayName"] = json!("Human model name");
    let old = ModelCatalog::parse(&serde_json::to_vec(&first).unwrap()).unwrap();
    first["fetchId"] = json!("rotated-fetch");
    first["rows"][0]["handle"] = json!("fbm1.rotated");
    first["rows"][0]["displayName"] = json!("New label");
    first["rows"][0]["compaction"] = policy(true);
    first["rows"][0]["legacyDigests"] = json!(["957291675c9f9132"]);
    let new = ModelCatalog::parse(&serde_json::to_vec(&first).unwrap()).unwrap();
    let old_pick = old.select("open").unwrap();
    let new_pick = new.select("open").unwrap();
    let old_root = old_pick.root_input();
    let new_root = new_pick.root_input();
    assert_eq!(old_pick.fetch_id, Some("synthetic-fetch"));
    assert_eq!(new_pick.fetch_id, Some("rotated-fetch"));
    assert_eq!(
        (old_root.model, old_root.model_label),
        ("fbm1.synthetic-open", "Human model name")
    );
    assert_eq!(
        (new_root.model, new_root.model_label),
        ("fbm1.rotated", "New label")
    );
    assert_eq!(
        old_root.compiled_model_id,
        Some("deepseek/deepseek-v4-flash")
    );
    assert_eq!(new_root.compiled_model_id, Some("upstage/solar-mini4"));
    assert_eq!(old_root.compaction.unwrap().lean, Some(false));
    assert_eq!(new_root.compaction.unwrap().lean, Some(true));
    assert!(std::ptr::eq(
        old_root.model.as_ptr(),
        old_pick.handle.as_ptr()
    ));
}

#[test]
fn freebuff_root_model_absent_partial_and_future_metadata_do_not_invent_policy() {
    let plain = catalog(None, &[]);
    assert!(plain
        .select("open")
        .unwrap()
        .root_input()
        .compaction
        .is_none());
    let extended = catalog(
        Some(json!({"cacheExpiryMs":1,"cacheExpiryMinTokens":0,
        "futurePolicy":{"supportedLater":true}})),
        &["", "future:digest"],
    );
    let root = extended.select("open").unwrap().root_input();
    let policy = root.compaction.unwrap();
    assert_eq!(
        (policy.cache_expiry_ms, policy.cache_expiry_min_tokens),
        (1, 0)
    );
    assert_eq!((policy.max_context_length, policy.lean), (None, None));
    assert_eq!(root.compiled_model_id, None);
    assert!(!root.turn_ending_note && !root.batching);
}

#[test]
fn freebuff_root_model_first_recognized_digest_is_not_a_union_of_model_flags() {
    for (digests, id, batching, note) in [
        (
            vec!["unknown", "6fe0bf9071b3f925", "1e303ac563a6f9cc"],
            "openai/gpt-6-luna",
            false,
            false,
        ),
        (
            vec!["unknown", "1e303ac563a6f9cc", "6fe0bf9071b3f925"],
            "deepseek/deepseek-v4-flash",
            true,
            false,
        ),
        (
            vec!["5ef411d6e9c19a25", "21dd0695d74d9a00"],
            "meta/muse-spark-1.2-contributor",
            false,
            false,
        ),
        (
            vec!["21dd0695d74d9a00", "5ef411d6e9c19a25"],
            "meta/muse-spark-1.3-contributor",
            false,
            true,
        ),
    ] {
        let catalog = catalog(None, &digests);
        let root = catalog.select("open").unwrap().root_input();
        assert_eq!(root.compiled_model_id, Some(id));
        assert_eq!((root.batching, root.turn_ending_note), (batching, note));
    }
}

#[test]
fn freebuff_root_model_all_compiled_flags_follow_lean_without_losing_row_policy() {
    for &(digest, id) in COMPILED_MODELS {
        for lean in [false, true] {
            let catalog = catalog(Some(policy(lean)), &[digest]);
            let root = catalog.select("open").unwrap().root_input();
            assert_eq!(root.compiled_model_id, Some(id));
            assert_eq!(
                root.batching,
                !lean
                    && matches!(
                        id,
                        "deepseek/deepseek-v4-flash" | "deepseek/deepseek-v4-flash-fast"
                    )
            );
            assert_eq!(
                root.turn_ending_note,
                matches!(
                    id,
                    "meta/muse-spark-1.3-contributor"
                        | "upstage/solar-pro4"
                        | "upstage/solar-mini4"
                )
            );
            assert_eq!(root.compaction.unwrap().max_context_length, Some(110000));
        }
    }
    let lean_without_budget = catalog(
        Some(json!({"cacheExpiryMs":1,
        "cacheExpiryMinTokens":0,"lean":true})),
        &["1e303ac563a6f9cc"],
    );
    let root = lean_without_budget.select("open").unwrap().root_input();
    assert!(!root.batching);
    assert_eq!(root.compaction.unwrap().max_context_length, None);
}

#[test]
fn freebuff_root_model_handle_label_and_unknown_digest_are_not_identity_or_debug_output() {
    let mut value = body();
    value["rows"][0]["handle"] = json!("fbm1.deepseek/deepseek-v4-flash");
    value["rows"][0]["displayName"] = json!("meta/muse-spark-1.3-contributor");
    value["rows"][0]["legacyDigests"] = json!(["deepseek/deepseek-v4-flash", "1E303AC563A6F9CC"]);
    let catalog = ModelCatalog::parse(&serde_json::to_vec(&value).unwrap()).unwrap();
    let root = catalog.select("open").unwrap().root_input();
    assert_eq!(root.compiled_model_id, None);
    assert!(!root.batching && !root.turn_ending_note);
    let debug = format!("{root:?}");
    assert!(!debug.contains(root.model) && !debug.contains(root.model_label));
    assert_eq!(
        catalog.select("locked").err(),
        Some(super::super::CatalogError::ModelUnavailable)
    );
    assert_eq!(
        catalog.select("future").err(),
        Some(super::super::CatalogError::ModelUnavailable)
    );
}

#[test]
fn freebuff_root_model_safe_numeric_and_digest_size_limits_accept_the_boundary() {
    let digest = "x".repeat(1024);
    let catalog = catalog(
        Some(json!({"cacheExpiryMs":9007199254740991u64,
        "cacheExpiryMinTokens":9007199254740991u64,"maxContextLength":9007199254740991u64})),
        &vec![digest.as_str(); 64],
    );
    let root = catalog.select("open").unwrap().root_input();
    assert_eq!(root.compiled_model_id, None);
    assert_eq!(
        root.compaction.unwrap().max_context_length,
        Some(9007199254740991)
    );
}

#[test]
fn freebuff_root_model_fixed_registry_and_utf16_digest_vectors_match_the_reference() {
    let expected_ids = [
        "stealth/ox-alpha",
        "stealth/space-bunny-alpha",
        "deepseek/deepseek-v4-pro",
        "minimax/minimax-m3",
        "openai/gpt-5.6-luna",
        "openai/gpt-6-luna",
        "openai/gpt-6.1-sol",
        "openai/gpt-6.1-terra",
        "google/gemini-4-argon",
        "upstage/solar-pro4",
        "upstage/solar-mini4",
        "google/gemini-3.8-flash",
        "meta/muse-spark-1.3-contributor",
        "meta/muse-spark-1.2-contributor",
        "z-ai/glm-5.2",
        "z-ai/glm-5.3-flash",
        "deepseek/deepseek-v4-flash",
        "deepseek/deepseek-v4-flash-fast",
        "mimo/mimo-v2.5",
        "mimo/mimo-v2.6-pro",
        "anthropic/claude-fable-5.1",
    ];
    assert_eq!(
        COMPILED_MODELS
            .iter()
            .map(|(_, id)| *id)
            .collect::<Vec<_>>(),
        expected_ids
    );
    for &(expected, id) in COMPILED_MODELS {
        assert_eq!(legacy_digest(id), expected, "{id}");
    }
    for (id, expected) in [
        ("", "ae7e16d8a2137f69"),
        ("模型😀", "1a7650b51ce23012"),
        ("deepseek/deepseek-v4-flash-2026-10-17", "bca3514b47f6ced0"),
    ] {
        assert_eq!(legacy_digest(id), expected);
    }
}
fn legacy_digest(id: &str) -> String {
    let (mut a, mut b) = (0x811c9dc5u32, 0x01000193u32 ^ 0x5bd1e995);
    for unit in "freebuff-legacy-model:"
        .encode_utf16()
        .chain(id.encode_utf16())
    {
        a = (a ^ u32::from(unit)).wrapping_mul(0x01000193);
        b = (b ^ u32::from(unit)).wrapping_mul(0x5bd1e995);
    }
    format!("{a:08x}{b:08x}")
}
