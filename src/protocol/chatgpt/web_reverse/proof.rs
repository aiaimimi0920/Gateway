//! Stable proof exports and original characterization test identities.
mod pow;
mod turnstile;
mod values;

pub use pow::{build_legacy_requirements_token, build_proof_token};
pub use turnstile::solve_turnstile_token;

#[cfg(test)]
use super::ChatGptWebBootstrap;
#[cfg(test)]
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
#[cfg(test)]
use pow::pow_generate;
#[cfg(test)]
use serde_json::{json, Value};
#[cfg(test)]
use values::xor_string;

#[cfg(test)]
mod tests {
    use super::*;

    fn bootstrap() -> ChatGptWebBootstrap {
        ChatGptWebBootstrap {
            pow_script_sources: vec!["https://chatgpt.com/backend-api/sentinel/sdk.js".to_string()],
            pow_data_build: Some("c/example/_".to_string()),
        }
    }

    #[test]
    fn build_legacy_requirements_token_returns_prefixed_token() {
        let token = build_legacy_requirements_token(&bootstrap(), "Mozilla/5.0 test");
        assert!(token.starts_with("gAAAAAC"));
        assert!(token.len() > "gAAAAAC".len());
    }

    #[test]
    fn pow_generate_emits_legacy_browser_json_shape() {
        let config = vec![
            json!(3000),
            json!("Mon Jan 02 2006 15:04:05 GMT-0500 (Eastern Standard Time)"),
            json!(4294705152u64),
            json!(0),
            json!("Mozilla/5.0 test"),
            json!("https://chatgpt.com/backend-api/sentinel/sdk.js"),
            json!("c/example/_"),
            json!("en-US"),
            json!("en-US,es-US,en,es"),
            json!(0),
            json!("vendor−Google Inc."),
            json!("location"),
            json!("window"),
            json!(123.0),
            json!("00000000-0000-0000-0000-000000000000"),
            json!(""),
            json!(8),
            json!(456.0),
        ];
        let (answer, solved) = pow_generate("seed", "ff", &config, 1);
        assert!(solved);
        let decoded = BASE64_STANDARD.decode(answer).expect("base64");
        let value: Value = serde_json::from_slice(&decoded).expect("legacy browser json array");
        let items = value.as_array().expect("array");
        assert_eq!(items.len(), 18);
        assert_eq!(items[3], json!(0));
        assert_eq!(items[9], json!(0));
        assert_eq!(items[4], json!("Mozilla/5.0 test"));
    }

    #[test]
    fn solve_turnstile_token_decodes_xored_program() {
        let p = "gAAAAAC-test-seed";
        let program = json!([[3, "turnstile-ok"]]);
        let program_text = serde_json::to_string(&program).expect("program json");
        let xored = xor_string(&program_text, p).expect("xor");
        let dx = BASE64_STANDARD.encode(xored.as_bytes());

        assert_eq!(
            solve_turnstile_token(&dx, p),
            Some(BASE64_STANDARD.encode("turnstile-ok"))
        );
    }

    #[test]
    fn solve_turnstile_token_handles_browser_vm_primitives() {
        let program = json!([
            [2, 30, "window"],
            [2, 31, "document"],
            [24, 32, 30, 31],
            [2, 33, "location"],
            [6, 34, 32, 33],
            [8, 35, 34],
            [19, 35],
            [20, 34, 34, 3, 35]
        ]);
        let program_text = serde_json::to_string(&program).expect("program json");
        let dx = BASE64_STANDARD.encode(program_text.as_bytes());
        let location = "https://chatgpt.com/";
        let once_encoded = BASE64_STANDARD.encode(location);

        assert_eq!(
            solve_turnstile_token(&dx, ""),
            Some(BASE64_STANDARD.encode(once_encoded))
        );
    }

    #[test]
    fn solve_turnstile_token_executes_second_stage_program() {
        let second_stage = json!([[3, "stage-ok"]]);
        let second_stage_text = serde_json::to_string(&second_stage).expect("stage json");
        let program = json!([
            [2, 20, BASE64_STANDARD.encode(second_stage_text.as_bytes())],
            [18, 20],
            [14, 9, 20]
        ]);
        let dx = BASE64_STANDARD.encode(
            serde_json::to_string(&program)
                .expect("program json")
                .as_bytes(),
        );

        assert_eq!(
            solve_turnstile_token(&dx, ""),
            Some(BASE64_STANDARD.encode("stage-ok"))
        );
    }

    #[test]
    fn build_proof_token_returns_prefixed_token_for_easy_difficulty() {
        let token =
            build_proof_token(&bootstrap(), "Mozilla/5.0 test", "seed", "ff").expect("proof token");
        assert!(token.starts_with("gAAAAAB"));
        assert!(token.len() > "gAAAAAB".len());
    }

    #[test]
    fn build_proof_token_errors_for_impossible_difficulty_window() {
        let error = build_proof_token(&bootstrap(), "Mozilla/5.0 test", "seed", "0000000000000000")
            .expect_err("proof failure");

        assert_eq!(
            error.code.as_deref(),
            Some("chatgpt_web_proof_token_failed")
        );
    }
}
