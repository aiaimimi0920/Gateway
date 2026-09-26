use super::http_probe::{build_absolute_url, probe_known_http_payload, ProbePolicy};
use super::{ProviderPayloadProbeReport, ProviderPayloadProbeStatus};
use crate::error::sanitize_provider_error_message;
use crate::routing::candidate::ProviderAccountPayload;

const CODEX_FIXED_SUPPORTED_MODELS: &[&str] =
    &["gpt-5.4", "gpt-5.4-mini", "gpt-5.3-codex", "gpt-5.2"];

pub async fn probe_provider_payload_for_console(
    client: &rquest::Client,
    payload: &ProviderAccountPayload,
) -> ProviderPayloadProbeReport {
    if payload_requires_browser_backing(payload) {
        return unsupported_probe_report(
            "Connectivity probe is unsupported for browser-backed credentials.",
        );
    }
    if fixed_models_for_payload(payload).is_some() {
        return unsupported_probe_report(
            "Connectivity probe is unsupported for fixed-model credentials.",
        );
    }
    if payload.canonical_adapter() == "search_api_compatible"
        && payload
            .balance_path
            .as_deref()
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .is_none()
    {
        return unsupported_probe_report(
            "Connectivity probe is unsupported for search_api credentials without an explicit balance_path.",
        );
    }

    match probe_known_http_payload(client, payload, ProbePolicy::Strict).await {
        Some(Ok(())) => ProviderPayloadProbeReport {
            status: ProviderPayloadProbeStatus::Passed,
            message: "Credential connectivity probe passed.".to_string(),
        },
        Some(Err(error)) => failed_probe_report(&error.message),
        None => {
            let adapter = payload.canonical_adapter();
            let message = match adapter {
                "freebuff_compatible" => {
                    "Connectivity probe is unsupported for stateful adapter 'freebuff_compatible'."
                        .to_string()
                }
                "udio_compatible"
                | "lumalabs_compatible"
                | "gemini_web_compatible"
                | "gemini_canvas_compatible"
                | "chatgpt_web_reverse_compatible"
                | "qwen_web_compatible"
                | "aistudio_web_reverse_compatible"
                | "suno_compatible" => {
                    format!("Connectivity probe is unsupported for browser adapter '{adapter}'.")
                }
                _ => format!("Connectivity probe is unsupported for adapter '{adapter}'."),
            };
            unsupported_probe_report(message)
        }
    }
}

pub fn provider_payload_probe_point(payload: &ProviderAccountPayload) -> String {
    if payload_requires_browser_backing(payload) {
        return "browser-backed credential (unsupported)".to_string();
    }
    if fixed_models_for_payload(payload).is_some() {
        return "fixed-model credential (unsupported)".to_string();
    }

    let base_url = payload.base_url.trim_end_matches('/');
    match payload.canonical_adapter() {
        "openai_compatible" | "anthropic_compatible" => {
            let url = format!("{base_url}/models");
            format!("GET {}", crate::console::secrets::redact_url_value(&url))
        }
        "search_api_compatible" => payload
            .balance_path
            .as_deref()
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(|path| {
                let url = build_absolute_url(base_url, path);
                format!("GET {}", crate::console::secrets::redact_url_value(&url))
            })
            .unwrap_or_else(|| "balance endpoint (unsupported)".to_string()),
        "producer_compatible" => {
            let url = format!("{base_url}/__api/billing/credits");
            format!("GET {}", crate::console::secrets::redact_url_value(&url))
        }
        adapter => format!("{adapter} connectivity point"),
    }
}

fn failed_probe_report(message: &str) -> ProviderPayloadProbeReport {
    let message = sanitize_provider_error_message(message);
    ProviderPayloadProbeReport {
        status: ProviderPayloadProbeStatus::Failed,
        message: if message.is_empty() {
            "Credential connectivity probe failed.".to_string()
        } else {
            message
        },
    }
}

fn unsupported_probe_report(message: impl Into<String>) -> ProviderPayloadProbeReport {
    ProviderPayloadProbeReport {
        status: ProviderPayloadProbeStatus::Unsupported,
        message: message.into(),
    }
}

fn payload_requires_browser_backing(payload: &ProviderAccountPayload) -> bool {
    let browser_backed = crate::routing::candidate::ProviderExecutionMode::BrowserBacked;
    payload.execution_mode == Some(browser_backed)
        || payload
            .endpoint_execution_modes
            .as_ref()
            .is_some_and(|modes| modes.values().any(|mode| *mode == browser_backed))
}

pub fn fixed_models_for_payload(payload: &ProviderAccountPayload) -> Option<Vec<String>> {
    if crate::protocol::chatgpt::official_api::is_chatgpt_codex_backend_payload(payload) {
        return Some(
            CODEX_FIXED_SUPPORTED_MODELS
                .iter()
                .map(|model| (*model).to_string())
                .collect(),
        );
    }
    None
}
