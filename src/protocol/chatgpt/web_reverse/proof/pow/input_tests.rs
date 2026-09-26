use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use serde_json::Value;
use sha3::{Digest, Sha3_512};

use crate::protocol::chatgpt::web_reverse::{
    build_legacy_requirements_token, build_proof_token, ChatGptWebBootstrap,
    CHATGPT_WEB_REVERSE_ADAPTER,
};

const SEED: &str = "pow-private-seed-sentinel";
const USER_AGENT: &str = "Gateway PoW contract fixture";
const SCRIPT: &str = "https://chatgpt.com/backend-api/sentinel/sdk.js";

fn bootstrap() -> ChatGptWebBootstrap {
    ChatGptWebBootstrap {
        pow_script_sources: vec![SCRIPT.to_string()],
        pow_data_build: Some("fixture-build".to_string()),
    }
}

fn assert_rejected(difficulty: &str) {
    let outcome =
        std::panic::catch_unwind(|| build_proof_token(&bootstrap(), USER_AGENT, SEED, difficulty));
    let error = outcome
        .expect("provided difficulty must not panic")
        .expect_err("provided invalid difficulty must not produce a proof");
    assert_eq!(error.http_status, Some(503));
    assert_eq!(
        error.code.as_deref(),
        Some("chatgpt_web_proof_token_failed")
    );
    assert_eq!(
        error.provider_name.as_deref(),
        Some(CHATGPT_WEB_REVERSE_ADAPTER)
    );
    assert!(error.message.len() <= 160, "diagnostic must remain bounded");
    assert!(!error.message.contains(SEED));
    assert!(!error.message.contains("PRIVATE_DIFFICULTY_MARKER"));
}

fn decode_browser_config(token: &str, prefix: &str) -> (String, Vec<Value>) {
    let encoded = token.strip_prefix(prefix).expect("stable token prefix");
    let decoded = BASE64_STANDARD.decode(encoded).expect("base64 proof");
    let items: Vec<Value> = serde_json::from_slice(&decoded).expect("browser JSON array");
    assert_eq!(items.len(), 18);
    assert_eq!(items[4].as_str(), Some(USER_AGENT));
    assert_eq!(items[5].as_str(), Some(SCRIPT));
    assert_eq!(items[6].as_str(), Some("fixture-build"));
    let nonce = items[3].as_u64().expect("nonce");
    assert!(nonce < 500_000);
    assert_eq!(items[9].as_u64(), Some(nonce >> 1));
    (encoded.to_string(), items)
}

fn assert_valid_proof(difficulty: &str) -> u64 {
    let token = build_proof_token(&bootstrap(), USER_AGENT, SEED, difficulty)
        .expect("valid challenge should solve");
    let (encoded, items) = decode_browser_config(&token, "gAAAAAB");
    // Verify the emitted proof, without reimplementing its nonce search.
    let mut verifier = Sha3_512::new();
    verifier.update(SEED.as_bytes());
    verifier.update(encoded.as_bytes());
    let hash = verifier.finalize();
    let target = hex::decode(difficulty).expect("fixture hex target");
    assert!(hash[..target.len()] <= target[..]);
    items[3].as_u64().unwrap()
}

#[test]
fn rejects_blank_difficulty() {
    assert_rejected("");
}

#[test]
fn rejects_odd_length_hex() {
    for difficulty in ["f", "fff", &"f".repeat(127)] {
        assert_rejected(difficulty);
    }
}

#[test]
fn rejects_non_hex_ascii() {
    for difficulty in ["gg", "0xff", "not-hex"] {
        assert_rejected(difficulty);
    }
}

#[test]
fn rejects_whitespace_without_trimming() {
    for difficulty in ["ff ", " ff", "\tff", "ff\n"] {
        assert_rejected(difficulty);
    }
}

#[test]
fn rejects_non_ascii_difficulty() {
    for difficulty in ["é", "ＦＦ", "零零"] {
        assert_rejected(difficulty);
    }
}

#[test]
fn rejects_target_wider_than_sha3_digest() {
    assert_rejected(&"ff".repeat(65));
}

#[test]
fn rejects_large_untrusted_difficulty_without_echo() {
    assert_rejected(&"PRIVATE_DIFFICULTY_MARKER".repeat(1024));
}

#[test]
fn preserves_hex_case_and_minimum_target_width() {
    for difficulty in ["ff", "FF", "fF"] {
        assert_eq!(assert_valid_proof(difficulty), 0);
    }
}

#[test]
fn preserves_maximum_digest_width() {
    assert_eq!(assert_valid_proof(&"ff".repeat(64)), 0);
}

#[test]
fn preserves_default_target_hash_and_nonce_contract() {
    assert_valid_proof("0fffff");
}

#[test]
fn preserves_legacy_requirements_wire_shape() {
    let token = build_legacy_requirements_token(&bootstrap(), USER_AGENT);
    decode_browser_config(&token, "gAAAAAC");
}
