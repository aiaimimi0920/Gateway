use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use rquest::header::{HeaderMap, HeaderName};
use serde_json::Value;

use crate::error::GatewayError;
use crate::object_storage::gateway_object_storage;
use crate::protocol::gemini_canvas;
use crate::upstream::aistudio::common::capture_contract::{
    target_rpc_contract_object_key, AIStudioTargetRpcContract,
};
use crate::upstream::aistudio::common::runtime_material::AIStudioWebReverseRuntimeMaterial;
use crate::upstream::common::insert_header_map_value;

const DEFAULT_AISTUDIO_ORIGIN: &str = "https://aistudio.google.com";
const DEFAULT_AISTUDIO_GRPC_USER_AGENT: &str = "grpc-web-javascript/0.1";
const DEFAULT_AISTUDIO_CONTENT_TYPE: &str = "application/json+protobuf";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AIStudioPureHttpMaterial {
    pub session: gemini_canvas::GeminiCanvasPureHttpSession,
    pub request_url: String,
    pub origin: String,
    pub referer: String,
    pub user_agent: String,
    pub content_type: String,
    pub x_goog_api_key: String,
    pub x_goog_authuser: String,
    pub x_user_agent: String,
    pub x_aistudio_g1_tier: Option<String>,
    pub x_aistudio_visit_id: Option<String>,
    pub x_browser_copyright: Option<String>,
    pub x_goog_ext_519733851_bin: Option<String>,
    pub x_browser_channel: Option<String>,
    pub x_browser_year: Option<String>,
    pub x_browser_validation: Option<String>,
    pub sec_fetch_mode: Option<String>,
    pub sec_fetch_site: Option<String>,
}

impl AIStudioPureHttpMaterial {
    pub async fn load_from_runtime(
        runtime: &AIStudioWebReverseRuntimeMaterial,
    ) -> Result<Option<(AIStudioTargetRpcContract, Self)>, GatewayError> {
        let Some(runtime_state_object_key) = runtime.runtime_state_object_key.as_deref() else {
            return Ok(None);
        };
        let contract_object_key = runtime
            .resolved_target_rpc_contract_object_key()
            .unwrap_or_else(|| target_rpc_contract_object_key(runtime_state_object_key));
        let object_storage = gateway_object_storage()?;
        let contract_value = match object_storage.read_json(&contract_object_key).await {
            Ok(value) => value,
            Err(_) => return Ok(None),
        };
        let contract = serde_json::from_value::<AIStudioTargetRpcContract>(contract_value)
            .map_err(|error| {
                GatewayError::server_error(format!(
                    "AI Studio target RPC contract at {} could not be parsed: {error}",
                    contract_object_key
                ))
                .with_code("aistudio_target_rpc_contract_parse_failed")
            })?;
        let storage_state = object_storage
            .read_json(runtime_state_object_key)
            .await
            .map_err(|error| {
                GatewayError::server_error(format!(
                    "AI Studio runtime state {} could not be loaded for pure HTTP material: {}",
                    runtime_state_object_key, error.message
                ))
                .with_code("aistudio_runtime_state_read_failed")
            })?;
        let material = Self::from_storage_state_and_contract(&storage_state, runtime, &contract)?;
        Ok(Some((contract, material)))
    }

    pub fn from_storage_state_and_contract(
        storage_state: &Value,
        runtime: &AIStudioWebReverseRuntimeMaterial,
        contract: &AIStudioTargetRpcContract,
    ) -> Result<Self, GatewayError> {
        let request_url = contract
            .preferred_request_url()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                GatewayError::service_unavailable(
                    "AI Studio target RPC contract did not include a request URL.",
                )
                .with_code("aistudio_target_rpc_contract_missing_url")
            })?
            .to_string();
        let origin = preferred_nonempty(
            contract.preferred_request_header("origin"),
            Some(DEFAULT_AISTUDIO_ORIGIN),
        )
        .unwrap()
        .to_string();
        let referer = preferred_nonempty(
            contract.preferred_request_header("referer"),
            Some(default_referer_for_origin(&origin).as_str()),
        )
        .unwrap()
        .to_string();
        let x_goog_authuser = preferred_nonempty(
            contract.preferred_request_header("x-goog-authuser"),
            Some("0"),
        )
        .unwrap()
        .to_string();
        let session = gemini_canvas::storage_state_to_pure_http_session(
            storage_state,
            &request_url,
            &runtime.app_url,
            &x_goog_authuser,
        )?;
        let x_goog_api_key = preferred_nonempty(
            contract.preferred_request_header("x-goog-api-key"),
            runtime.cloud_api_key.as_deref(),
        )
        .ok_or_else(|| {
            GatewayError::service_unavailable(
                "AI Studio pure HTTP material requires an x-goog-api-key from capture contract or cloudApiKey runtime material.",
            )
            .with_code("aistudio_pure_http_missing_api_key")
        })?
        .to_string();

        Ok(Self {
            session,
            request_url,
            origin: origin.clone(),
            referer,
            user_agent: preferred_nonempty(
                contract.preferred_request_header("user-agent"),
                Some(gemini_canvas::GEMINI_CANVAS_BROWSER_USER_AGENT),
            )
            .unwrap()
            .to_string(),
            content_type: preferred_nonempty(
                contract.preferred_request_header("content-type"),
                Some(DEFAULT_AISTUDIO_CONTENT_TYPE),
            )
            .unwrap()
            .to_string(),
            x_goog_api_key,
            x_goog_authuser,
            x_user_agent: preferred_nonempty(
                contract.preferred_request_header("x-user-agent"),
                Some(DEFAULT_AISTUDIO_GRPC_USER_AGENT),
            )
            .unwrap()
            .to_string(),
            x_aistudio_g1_tier: normalize_optional(
                contract.preferred_request_header("x-aistudio-g1-tier"),
            ),
            x_aistudio_visit_id: normalize_optional(
                contract.preferred_request_header("x-aistudio-visit-id"),
            ),
            x_browser_copyright: normalize_optional(
                contract.preferred_request_header("x-browser-copyright"),
            )
            .or_else(|| Some(gemini_canvas::GEMINI_CANVAS_BROWSER_COPYRIGHT.to_string())),
            x_goog_ext_519733851_bin: normalize_optional(
                contract.preferred_request_header("x-goog-ext-519733851-bin"),
            ),
            x_browser_channel: normalize_optional(
                contract.preferred_request_header("x-browser-channel"),
            )
            .or_else(|| Some(gemini_canvas::GEMINI_CANVAS_BROWSER_CHANNEL.to_string())),
            x_browser_year: normalize_optional(contract.preferred_request_header("x-browser-year"))
                .or_else(|| Some(gemini_canvas::GEMINI_CANVAS_BROWSER_YEAR.to_string())),
            x_browser_validation: normalize_optional(
                contract.preferred_request_header("x-browser-validation"),
            )
            .or_else(|| Some(gemini_canvas::GEMINI_CANVAS_BROWSER_VALIDATION.to_string())),
            sec_fetch_mode: normalize_optional(contract.preferred_request_header("sec-fetch-mode"))
                .or_else(|| Some("cors".to_string())),
            sec_fetch_site: normalize_optional(contract.preferred_request_header("sec-fetch-site"))
                .or_else(|| Some("same-site".to_string())),
        })
    }

    pub fn build_capture_aligned_headers(
        &self,
        timestamp_secs: Option<i64>,
    ) -> Result<HeaderMap, GatewayError> {
        let timestamp_secs = timestamp_secs.unwrap_or_else(current_unix_timestamp_secs);
        let authorization = gemini_canvas::build_sapisid_authorization(
            &self.session.sapisid,
            &self.origin,
            timestamp_secs,
        )?;
        let mut headers = HeaderMap::new();
        for name in [
            "authorization",
            "cookie",
            "content-type",
            "origin",
            "referer",
            "x-goog-api-key",
            "x-goog-authuser",
            "x-user-agent",
            "user-agent",
            "x-aistudio-visit-id",
            "x-aistudio-g1-tier",
            "x-goog-ext-519733851-bin",
            "x-browser-channel",
            "x-browser-copyright",
            "x-browser-validation",
            "x-browser-year",
            "accept",
            "accept-language",
            "sec-fetch-mode",
            "sec-fetch-site",
        ] {
            headers.remove(HeaderName::from_static(name));
        }
        insert_header_map_value(&mut headers, "authorization", &authorization);
        insert_header_map_value(&mut headers, "cookie", &self.session.cookie_header);
        insert_header_map_value(&mut headers, "content-type", &self.content_type);
        insert_header_map_value(&mut headers, "origin", &self.origin);
        insert_header_map_value(&mut headers, "referer", &self.referer);
        insert_header_map_value(&mut headers, "x-goog-api-key", &self.x_goog_api_key);
        insert_header_map_value(&mut headers, "x-goog-authuser", &self.x_goog_authuser);
        insert_header_map_value(&mut headers, "x-user-agent", &self.x_user_agent);
        insert_header_map_value(&mut headers, "user-agent", &self.user_agent);
        insert_header_map_value(&mut headers, "accept", "*/*");
        insert_header_map_value(&mut headers, "accept-language", "zh-CN");
        if let Some(value) = self.x_aistudio_g1_tier.as_deref() {
            insert_header_map_value(&mut headers, "x-aistudio-g1-tier", value);
        }
        if let Some(value) = self.x_aistudio_visit_id.as_deref() {
            insert_header_map_value(&mut headers, "x-aistudio-visit-id", value);
        }
        if let Some(value) = self.x_browser_copyright.as_deref() {
            insert_header_map_value(&mut headers, "x-browser-copyright", value);
        }
        if let Some(value) = self.x_goog_ext_519733851_bin.as_deref() {
            insert_header_map_value(&mut headers, "x-goog-ext-519733851-bin", value);
        }
        if let Some(value) = self.x_browser_channel.as_deref() {
            insert_header_map_value(&mut headers, "x-browser-channel", value);
        }
        if let Some(value) = self.x_browser_validation.as_deref() {
            insert_header_map_value(&mut headers, "x-browser-validation", value);
        }
        if let Some(value) = self.x_browser_year.as_deref() {
            insert_header_map_value(&mut headers, "x-browser-year", value);
        }
        if let Some(value) = self.sec_fetch_mode.as_deref() {
            insert_header_map_value(&mut headers, "sec-fetch-mode", value);
        }
        if let Some(value) = self.sec_fetch_site.as_deref() {
            insert_header_map_value(&mut headers, "sec-fetch-site", value);
        }
        Ok(headers)
    }

    pub fn build_capture_aligned_header_map(
        &self,
        timestamp_secs: Option<i64>,
    ) -> Result<HashMap<String, String>, GatewayError> {
        let header_map = self.build_capture_aligned_headers(timestamp_secs)?;
        let mut result = HashMap::new();
        for (name, value) in &header_map {
            if let Ok(as_str) = value.to_str() {
                result.insert(name.as_str().to_string(), as_str.to_string());
            }
        }
        Ok(result)
    }
}

fn normalize_optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string)
}

fn preferred_nonempty<'a>(primary: Option<&'a str>, fallback: Option<&'a str>) -> Option<&'a str> {
    primary
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| fallback.map(str::trim).filter(|value| !value.is_empty()))
}

fn default_referer_for_origin(origin: &str) -> String {
    let trimmed = origin.trim_end_matches('/');
    format!("{trimmed}/")
}

fn current_unix_timestamp_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::upstream::aistudio::common::capture_contract::{
        AIStudioTargetRpcContract, AIStudioTargetRpcEndpointContract,
    };

    fn make_storage_state() -> Value {
        serde_json::json!({
            "cookies": [
                {
                    "name": "SAPISID",
                    "value": "sapisid-123",
                    "domain": ".google.com",
                    "path": "/",
                    "secure": true
                },
                {
                    "name": "__Secure-1PSID",
                    "value": "psid-123",
                    "domain": ".google.com",
                    "path": "/",
                    "secure": true
                }
            ]
        })
    }

    fn make_runtime_material() -> AIStudioWebReverseRuntimeMaterial {
        AIStudioWebReverseRuntimeMaterial {
            runtime_state_object_key: Some(
                "credential-runtime/aistudio/demo/storage-state.json".to_string(),
            ),
            target_rpc_contract_object_key: None,
            app_url: "https://ai.studio/apps/example".to_string(),
            browser_executable_path: None,
            cloud_api_key: Some("AIzaSyRuntimeFallback".to_string()),
        }
    }

    #[test]
    fn pure_http_material_uses_contract_headers_and_runtime_fallbacks() {
        let contract = AIStudioTargetRpcContract {
            code_assistant_offline: AIStudioTargetRpcEndpointContract {
                url: Some(
                    "https://alkalimakersuite-pa.clients6.google.com/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/CodeAssistantOffline"
                        .to_string(),
                ),
                request_headers: HashMap::from([
                    (
                        "origin".to_string(),
                        "https://aistudio.google.com".to_string(),
                    ),
                    (
                        "referer".to_string(),
                        "https://aistudio.google.com/".to_string(),
                    ),
                    (
                        "x-goog-authuser".to_string(),
                        "0".to_string(),
                    ),
                    (
                        "x-aistudio-visit-id".to_string(),
                        "visit-123".to_string(),
                    ),
                ]),
                ..Default::default()
            },
            ..Default::default()
        };

        let material = AIStudioPureHttpMaterial::from_storage_state_and_contract(
            &make_storage_state(),
            &make_runtime_material(),
            &contract,
        )
        .expect("material should build");

        assert_eq!(material.x_goog_api_key, "AIzaSyRuntimeFallback");
        assert_eq!(material.x_goog_authuser, "0");
        assert_eq!(material.x_aistudio_visit_id.as_deref(), Some("visit-123"));
        assert!(material
            .session
            .cookie_header
            .contains("SAPISID=sapisid-123"));
    }

    #[test]
    fn capture_aligned_headers_include_authorization_and_cookie() {
        let contract = AIStudioTargetRpcContract {
            code_assistant_offline: AIStudioTargetRpcEndpointContract {
                url: Some(
                    "https://alkalimakersuite-pa.clients6.google.com/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/CodeAssistantOffline"
                        .to_string(),
                ),
                request_headers: HashMap::from([(
                    "x-goog-api-key".to_string(),
                    "AIzaSyCaptured".to_string(),
                )]),
                ..Default::default()
            },
            ..Default::default()
        };
        let material = AIStudioPureHttpMaterial::from_storage_state_and_contract(
            &make_storage_state(),
            &make_runtime_material(),
            &contract,
        )
        .expect("material should build");
        let headers = material
            .build_capture_aligned_header_map(Some(1_700_000_000))
            .expect("headers should build");
        assert_eq!(
            headers.get("authorization").map(String::as_str),
            Some(
                "SAPISIDHASH 1700000000_407b8e1983177c4befc8aa5296db5073bd585c04 SAPISID1PHASH 1700000000_407b8e1983177c4befc8aa5296db5073bd585c04 SAPISID3PHASH 1700000000_407b8e1983177c4befc8aa5296db5073bd585c04"
            )
        );
        assert_eq!(
            headers.get("x-goog-api-key").map(String::as_str),
            Some("AIzaSyCaptured")
        );
        assert!(headers
            .get("cookie")
            .is_some_and(|value| value.contains("SAPISID=sapisid-123")));
    }
}
