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
                format!("{base_url}/models"),
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

pub(super) async fn send_probe_request(
    client: &rquest::Client,
    method: Method,
    url: String,
    payload: &ProviderAccountPayload,
    expectation: ProbeExpectation,
) -> Result<(), GatewayError> {
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
