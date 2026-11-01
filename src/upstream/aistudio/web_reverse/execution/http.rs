//! Playground HTTP execution with bounded, cancellation-owned native proof processes.
use std::path::PathBuf;
use std::sync::LazyLock;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::sync::Semaphore;

use crate::error::GatewayError;
use crate::object_storage::gateway_object_storage;
use crate::protocol::aistudio_web;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::client::UpstreamClient;
use crate::upstream::producer_browser_worker_process::{
    execute_producer_browser_worker_process, PreparedProducerBrowserWorkerLaunch,
};

static PROOF_SLOTS: LazyLock<Semaphore> = LazyLock::new(|| Semaphore::new(2));

fn failure(code: &str) -> GatewayError {
    let mut error = GatewayError::service_unavailable(
        "AI Studio HTTP request failed; private details were omitted.",
    )
    .with_provider(aistudio_web::AISTUDIO_WEB_REVERSE_ADAPTER)
    .with_code(code);
    // GenerateContent has no idempotency key: a lost response may still have generated.
    error.retryable = false;
    error.fallback_hint = crate::error::FallbackHint::Abort {
        reason: "AI Studio generation cannot be safely replayed.".to_string(),
    };
    error
}

fn script_path() -> PathBuf {
    let bundled = std::env::current_exe().ok().and_then(|exe| {
        exe.parent()
            .map(|dir| dir.join("scripts/aistudio-http-worker.mjs"))
    });
    bundled.filter(|path| path.is_file()).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/aistudio-http-worker.mjs")
    })
}

fn admit_origin(base: &str) -> Result<(), GatewayError> {
    let url = url::Url::parse(base).map_err(|_| failure("aistudio_http_origin_invalid"))?;
    if url.scheme() != "https"
        || !matches!(url.host_str(), Some("aistudio.google.com" | "ai.studio"))
        || url.port_or_known_default() != Some(443)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "" | "/")
    {
        return Err(failure("aistudio_http_origin_invalid"));
    }
    Ok(())
}

impl UpstreamClient {
    pub(crate) async fn execute_aistudio_http_text(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        body: &Value,
    ) -> Result<Value, GatewayError> {
        self.execute_aistudio_http_content(payload, model, body, None)
            .await
    }

    pub(super) async fn execute_aistudio_http_content(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        body: &Value,
        output_format: Option<&str>,
    ) -> Result<Value, GatewayError> {
        admit_origin(&payload.base_url)?;
        let timeout = self.timeout.min(Duration::from_secs(180));
        tokio::time::timeout(timeout, async {
            let _permit = PROOF_SLOTS
                .acquire()
                .await
                .map_err(|_| failure("aistudio_http_capacity_unavailable"))?;
            let key = payload
                .runtime_state_object_key
                .as_deref()
                .filter(|key| !key.trim().is_empty())
                .ok_or_else(|| failure("aistudio_http_cookie_import_required"))?;
            let storage =
                gateway_object_storage().map_err(|_| failure("aistudio_http_state_unavailable"))?;
            let bytes = storage
                .read_bytes_with_limit(key, 32768)
                .await
                .map_err(|_| failure("aistudio_http_state_unavailable"))?;
            let cookies: Value = serde_json::from_slice(&bytes)
                .map_err(|_| failure("aistudio_http_cookie_material_invalid"))?;
            let input = serde_json::to_vec(&json!({"cookies":cookies, "body":body,
                "model":model, "timeoutMs":timeout.as_millis(), "outputFormat":output_format}))
            .map_err(|_| failure("aistudio_http_input_invalid"))?;
            if input.len() > 9 * 1024 * 1024 {
                return Err(failure("aistudio_http_input_limit"));
            }
            // Reuse only the generic process-tree supervisor: this script never imports a browser.
            let output = execute_producer_browser_worker_process(
                PreparedProducerBrowserWorkerLaunch {
                    node_bin: std::env::var("AISTUDIO_HTTP_NODE_BIN")
                        .unwrap_or_else(|_| "node".to_string()),
                    script_path: script_path(),
                    stdin_json: input,
                },
                timeout,
            )
            .await
            .map_err(|_| failure("aistudio_http_worker_failed"))?;
            let value: Value = serde_json::from_slice(&output.stdout)
                .map_err(|_| failure("aistudio_http_worker_output_invalid"))?;
            if value.get("ok").and_then(Value::as_bool) != Some(true) {
                let code = value
                    .get("code")
                    .and_then(Value::as_str)
                    .filter(|s| {
                        s.starts_with("aistudio_http_")
                            && s.len() <= 96
                            && s.bytes().all(|c| c.is_ascii_lowercase() || c == b'_')
                    })
                    .unwrap_or("aistudio_http_request_failed");
                return Err(failure(code));
            }
            value
                .get("response")
                .filter(|v| v.is_object())
                .cloned()
                .ok_or_else(|| failure("aistudio_http_response_missing"))
        })
        .await
        .map_err(|_| failure("aistudio_http_deadline_exceeded"))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ambiguous_generation_failure_cannot_be_retried_or_rerouted() {
        let error = failure("aistudio_http_worker_failed");
        assert!(!error.retryable);
        assert!(matches!(
            error.fallback_hint,
            crate::error::FallbackHint::Abort { .. }
        ));
    }

    #[test]
    fn only_canonical_and_legacy_root_origins_are_admitted() {
        for base in ["https://aistudio.google.com", "https://ai.studio/"] {
            assert!(admit_origin(base).is_ok());
        }
        for base in [
            "http://aistudio.google.com",
            "https://aistudio.google.com.evil.example",
            "https://user:secret@aistudio.google.com",
            "https://aistudio.google.com/apps/id",
            "https://aistudio.google.com/?token=secret",
        ] {
            assert!(admit_origin(base).is_err());
        }
    }
}
