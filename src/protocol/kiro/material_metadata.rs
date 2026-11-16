//! Pure legacy material lookup and device identity, identical with the Kiro line disabled.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

pub fn read_payload_string(
    extra: Option<&HashMap<String, Value>>,
    aliases: &[&str],
) -> Option<String> {
    let extra = extra?;
    let alias_keys: Vec<String> = aliases
        .iter()
        .map(|key| normalize_lookup_key(key))
        .collect();
    extra.iter().find_map(|(key, value)| {
        if !alias_keys.contains(&normalize_lookup_key(key)) {
            return None;
        }
        match value {
            Value::String(text) => {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }
            Value::Number(number) => Some(number.to_string()),
            Value::Bool(flag) => Some(flag.to_string()),
            _ => None,
        }
    })
}

pub fn normalize_machine_id(machine_id: &str) -> Option<String> {
    let trimmed = machine_id.trim();
    if trimmed.len() == 64 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        return Some(trimmed.to_ascii_lowercase());
    }
    let without_dashes: String = trimmed.chars().filter(|c| *c != '-').collect();
    if without_dashes.len() == 32 && without_dashes.chars().all(|c| c.is_ascii_hexdigit()) {
        let normalized = without_dashes.to_ascii_lowercase();
        return Some(format!("{normalized}{normalized}"));
    }
    None
}

pub fn generate_machine_id(
    explicit_machine_id: Option<&str>,
    refresh_token: Option<&str>,
) -> Option<String> {
    if let Some(machine_id) = explicit_machine_id.and_then(normalize_machine_id) {
        return Some(machine_id);
    }
    let refresh_token = refresh_token?.trim();
    if refresh_token.is_empty() {
        return None;
    }
    Some(hex::encode(Sha256::digest(
        format!("KotlinNativeAPI/{refresh_token}").as_bytes(),
    )))
}

pub(super) fn normalize_lookup_key(value: &str) -> String {
    value
        .chars()
        .filter(|ch| *ch != '_' && *ch != '-')
        .collect::<String>()
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn kiro_material_machine_normalization_preserves_legacy_values() {
        let uuid = "ABCDEF01-2345-6789-ABCD-EF0123456789";
        let expected = "abcdef0123456789abcdef0123456789".repeat(2);
        assert_eq!(
            normalize_machine_id(uuid).as_deref(),
            Some(expected.as_str())
        );
        assert_eq!(
            normalize_machine_id(&expected.to_uppercase()),
            Some(expected)
        );
        for invalid in ["", "bad", &"a".repeat(63), &"g".repeat(64)] {
            assert!(normalize_machine_id(invalid).is_none());
        }
    }

    #[test]
    fn kiro_material_derivation_preserves_exact_hash_and_explicit_precedence() {
        assert_eq!(
            generate_machine_id(None, Some(" refresh-token-123 ")).as_deref(),
            Some("145edc079178d090255a363ccbe2a0d8b85fa366102126f35a4219b6954d9cd0")
        );
        let machine = "a".repeat(64);
        assert_eq!(
            generate_machine_id(Some(&machine), Some("changed-rt")),
            Some(machine)
        );
        assert!(generate_machine_id(None, Some(" ")).is_none());
        assert!(generate_machine_id(None, None).is_none());
    }

    #[test]
    fn kiro_material_lookup_retains_alias_and_scalar_contract() {
        for (input, expected) in [
            (json!("  explicit-machine  "), Some("explicit-machine")),
            (json!(123), Some("123")),
            (json!(false), Some("false")),
            (json!("  "), None),
            (Value::Null, None),
            (json!({}), None),
        ] {
            let extra = HashMap::from([("KIRO-machine_ID".into(), input)]);
            assert_eq!(
                read_payload_string(Some(&extra), &["kiroMachineId"]).as_deref(),
                expected
            );
        }
    }
}
