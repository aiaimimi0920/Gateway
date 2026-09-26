//! Build Qwen browser worker inputs without changing provider payload state.
use super::signin::qwen_web_signin_seed;
use super::types::{QwenWebSessionWorkerAuthSeed, QwenWebSessionWorkerInput};
use crate::routing::candidate::ProviderAccountPayload;

const QWEN_WEB_CREDENTIAL_FAMILY_DIR: &str = "qwen-web-chat";

fn sanitize_qwen_web_credential_file_name(value: &str) -> Option<String> {
    let normalized = value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|ch| match ch {
            'a'..='z' | '0'..='9' | '.' | '_' | '-' => ch,
            _ => '-',
        })
        .collect::<String>();
    let collapsed = normalized
        .split('-')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if collapsed.is_empty() {
        None
    } else if collapsed.ends_with(".json") {
        Some(collapsed)
    } else {
        Some(format!("{collapsed}.json"))
    }
}

pub(super) fn qwen_web_refresh_input(
    payload: &ProviderAccountPayload,
    model: &str,
) -> QwenWebSessionWorkerInput {
    let preferred_models = if model.trim().is_empty() {
        Vec::new()
    } else {
        vec![model.trim().to_string()]
    };
    let credential_file_name = payload
        .credential_id
        .as_deref()
        .and_then(sanitize_qwen_web_credential_file_name)
        .or_else(|| {
            payload
                .account_name
                .as_deref()
                .and_then(sanitize_qwen_web_credential_file_name)
        });

    QwenWebSessionWorkerInput {
        base_url: payload.base_url.trim().trim_end_matches('/').to_string(),
        preferred_models,
        write_credential_file: true,
        credential_file_name,
        credential_family_dir: Some(QWEN_WEB_CREDENTIAL_FAMILY_DIR.to_string()),
        credential_root_dir: std::env::var("NEURO_PROVIDER_CREDENTIAL_ROOT_DIR")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        browser_executable_path: std::env::var("QWEN_WEB_BROWSER_EXECUTABLE_PATH")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        user_data_dir: std::env::var("QWEN_WEB_BROWSER_USER_DATA_DIR")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        profile_directory: std::env::var("QWEN_WEB_BROWSER_PROFILE_DIR")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        auth_seed: qwen_web_signin_seed(payload.extra_body.as_ref()).map(|seed| {
            QwenWebSessionWorkerAuthSeed {
                email: seed.email,
                password: seed.password,
                password_sha256: seed.password_sha256,
            }
        }),
    }
}
