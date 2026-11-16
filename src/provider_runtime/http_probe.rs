use std::time::Duration;

use rquest::Method;

use crate::error::GatewayError;
use crate::routing::candidate::ProviderAccountPayload;
use crate::state::AppState;
use crate::upstream::headers::build_upstream_headers;

pub(super) fn build_probe_http_client(state: &AppState) -> Result<rquest::Client, GatewayError> {
    crate::http_client::builder()
        .timeout(Duration::from_secs(state.config.upstream_timeout_secs))
        .build()
        .map_err(|error| GatewayError::server_error(format!("build probe http client: {error}")))
}

#[derive(Clone, Copy)]
pub(super) enum ProbePolicy {
    Legacy,
    Strict,
}

pub(super) async fn probe_known_http_payload(
    client: &rquest::Client,
    payload: &ProviderAccountPayload,
    policy: ProbePolicy,
) -> Option<Result<(), GatewayError>> {
    let base_url = payload.base_url.trim_end_matches('/').to_string();
    let strict_expectation = ProbeExpectation::HttpOk;
    let reachability_expectation = match policy {
        ProbePolicy::Legacy => ProbeExpectation::AllowClientErrors,
        ProbePolicy::Strict => ProbeExpectation::HttpOk,
    };

    match payload.canonical_adapter() {
        "openai_compatible" | "anthropic_compatible" => Some(
            send_probe_request(
                client,
                Method::GET,
                if payload.canonical_adapter() == "anthropic_compatible" {
                    format!("{base_url}/v1/models")
                } else {
                    format!("{base_url}/models")
                },
                payload,
                strict_expectation,
            )
            .await,
        ),
        "grok_compatible" => match policy {
            ProbePolicy::Legacy => Some(
                send_probe_request(
                    client,
                    Method::GET,
                    base_url,
                    payload,
                    reachability_expectation,
                )
                .await,
            ),
            ProbePolicy::Strict => None,
        },
        "search_api_compatible" => {
            let path = match policy {
                ProbePolicy::Legacy => payload
                    .balance_path
                    .clone()
                    .or_else(|| payload.search_path.clone())
                    .unwrap_or_else(|| "/v1/credits/balance".to_string()),
                ProbePolicy::Strict => payload
                    .balance_path
                    .as_deref()
                    .map(str::trim)
                    .filter(|path| !path.is_empty())
                    .map(str::to_string)?,
            };
            Some(
                send_probe_request(
                    client,
                    Method::GET,
                    build_absolute_url(&base_url, &path),
                    payload,
                    strict_expectation,
                )
                .await,
            )
        }
        "producer_compatible" => Some(
            send_probe_request(
                client,
                Method::GET,
                format!("{base_url}/__api/billing/credits"),
                payload,
                strict_expectation,
            )
            .await,
        ),
        "custom_http" | "provider_passthrough" => match policy {
            ProbePolicy::Legacy => {
                let head_result = send_probe_request(
                    client,
                    Method::HEAD,
                    base_url.clone(),
                    payload,
                    reachability_expectation,
                )
                .await;
                Some(match head_result {
                    Ok(()) => Ok(()),
                    Err(_error) => {
                        send_probe_request(
                            client,
                            Method::GET,
                            base_url,
                            payload,
                            reachability_expectation,
                        )
                        .await
                    }
                })
            }
            ProbePolicy::Strict => None,
        },
        _ => None,
    }
}

#[derive(Clone, Copy)]
pub(super) enum ProbeExpectation {
    HttpOk,
    AllowClientErrors,
}

#[cfg(test)]
#[tokio::test]
async fn luma_mcp_generic_probe_is_rejected_before_send() {
    let payload = serde_json::from_value(serde_json::json!({
        "adapter":"lumalabs_mcp_readonly","base_url":"http://127.0.0.1:1",
        "api_key":"synthetic"}))
    .unwrap();
    let error = send_probe_request(
        &rquest::Client::new(),
        Method::GET,
        "http://127.0.0.1:1/probe".into(),
        &payload,
        ProbeExpectation::HttpOk,
    )
    .await
    .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("luma_mcp_management_only"));
}

pub(super) async fn send_probe_request(
    client: &rquest::Client,
    method: Method,
    url: String,
    payload: &ProviderAccountPayload,
    expectation: ProbeExpectation,
) -> Result<(), GatewayError> {
    crate::protocol::lobsterai::management_boundary::reject_generic(payload)?;
    crate::protocol::luma_mcp::boundary::reject_generic(payload)?;
    if crate::protocol::commandcode::owns_payload(payload) {
        return Err(crate::protocol::commandcode::invalid());
    }
    if crate::protocol::sap_aicore::owns_payload(payload) {
        return Err(crate::protocol::sap_aicore::invalid());
    }
    if crate::protocol::kiro_oauth::runtime::owns_payload(payload) {
        return Err(crate::protocol::kiro_oauth::runtime::invalid());
    }
    if crate::protocol::kiro_api_key::owns_payload(payload) {
        return Err(crate::protocol::kiro_api_key::invalid());
    }
    if crate::protocol::grok_oauth::business::owns_payload(payload) {
        return Err(crate::protocol::grok_oauth::business::invalid());
    }
    if crate::protocol::vertex_express::owns_payload(payload) {
        return Err(crate::protocol::vertex_express::invalid());
    }
    let response = client
        .request(method, url)
        .headers(build_upstream_headers(payload))
        .send()
        .await
        .map_err(|error| {
            GatewayError::service_unavailable(format!("provider probe request failed: {error}"))
        })?;

    match expectation {
        ProbeExpectation::HttpOk if !response.status().is_success() => Err(GatewayError::conflict(
            format!("Provider probe failed with status {}.", response.status()),
        )),
        ProbeExpectation::AllowClientErrors if response.status().as_u16() >= 500 => {
            Err(GatewayError::conflict(format!(
                "Provider probe failed with status {}.",
                response.status()
            )))
        }
        _ => Ok(()),
    }
}

pub(super) fn build_absolute_url(base_url: &str, path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        return path.to_string();
    }
    if path.starts_with('/') {
        format!("{base_url}{path}")
    } else {
        format!("{base_url}/{path}")
    }
}

#[cfg(test)]
mod grok_oauth_tests {
    use super::*;
    #[tokio::test]
    async fn commandcode_generic_probe_rejects_before_network() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let payload: ProviderAccountPayload = serde_json::from_value(serde_json::json!({
            "adapter":"openai_compatible","base_url":base,"api_key":"synthetic-commandcode",
            "auth_mode":crate::protocol::commandcode::AUTH_MODE}))
        .unwrap();
        let client = rquest::Client::builder().no_proxy().build().unwrap();
        let error = send_probe_request(
            &client,
            Method::GET,
            base,
            &payload,
            ProbeExpectation::HttpOk,
        )
        .await
        .unwrap_err();
        assert_eq!(error.code.as_deref(), Some("invalid_commandcode_chat"));
        assert!(
            tokio::time::timeout(Duration::from_millis(50), listener.accept())
                .await
                .is_err()
        );
    }
    #[tokio::test]
    async fn grok_oauth_generic_probes_never_send_a_token_or_report_connectivity_success() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let payload: ProviderAccountPayload = serde_json::from_value(serde_json::json!({
            "adapter":"openai_compatible","base_url":base,"api_key":"synthetic-build-only",
            "auth_mode":crate::protocol::grok_oauth::business::AUTH_MODE,
        }))
        .unwrap();
        let client = rquest::Client::builder().no_proxy().build().unwrap();
        for policy in [ProbePolicy::Legacy, ProbePolicy::Strict] {
            assert!(probe_known_http_payload(&client, &payload, policy)
                .await
                .unwrap()
                .is_err());
        }
        assert!(send_probe_request(
            &client,
            Method::POST,
            base.clone(),
            &payload,
            ProbeExpectation::HttpOk
        )
        .await
        .is_err());
        let report =
            crate::provider_runtime::probe_provider_payload_for_console(&client, &payload).await;
        assert!(matches!(
            report.status,
            crate::provider_runtime::ProviderPayloadProbeStatus::Unsupported
        ));
        assert!(
            crate::provider_runtime::provider_payload_probe_point(&payload)
                .starts_with("Grok Build")
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(100), listener.accept())
                .await
                .is_err()
        );
    }
}
