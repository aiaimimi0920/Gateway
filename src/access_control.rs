use std::collections::HashSet;

use axum::http::header::AUTHORIZATION;
use axum::http::HeaderMap;

use crate::error::GatewayError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InternalAccessSurface {
    Management,
    BrowserExecutor,
}

impl InternalAccessSurface {
    fn missing_token_error(self) -> GatewayError {
        match self {
            Self::Management => {
                GatewayError::service_unavailable("GATEWAY_MANAGEMENT_TOKEN is not configured")
                    .with_code("gateway_management_token_not_configured")
            }
            Self::BrowserExecutor => GatewayError::service_unavailable(
                "GATEWAY_BROWSER_EXECUTOR_BEARER_TOKEN is not configured",
            )
            .with_code("browser_executor_token_not_configured"),
        }
    }

    fn unauthorized_error(self) -> GatewayError {
        match self {
            Self::Management => GatewayError::unauthorized("Management token is required"),
            Self::BrowserExecutor => {
                GatewayError::unauthorized("Browser executor token is required")
            }
        }
    }
}

pub fn authorize_internal_request(
    surface: InternalAccessSurface,
    expected_token: Option<&str>,
    allow_unauthenticated: bool,
    bearer_token: Option<&str>,
    headers: &HeaderMap,
) -> Result<(), GatewayError> {
    let Some(expected_token) = normalized(expected_token) else {
        if allow_unauthenticated {
            tracing::warn!(
                surface = ?surface,
                "Allowing an unauthenticated internal route because its explicit development override is enabled"
            );
            return Ok(());
        }
        return Err(surface.missing_token_error());
    };

    let surface_header = match surface {
        InternalAccessSurface::Management => header_value(headers, "x-management-token"),
        InternalAccessSurface::BrowserExecutor => None,
    };
    let provided = surface_header
        .or_else(|| header_value(headers, "x-internal-api-key"))
        .or_else(|| normalized(bearer_token))
        .or_else(|| bearer_from_authorization(headers));

    match provided {
        Some(token) if token == expected_token => Ok(()),
        _ => Err(surface.unauthorized_error()),
    }
}

pub fn unauthenticated_internal_routes_allowed() -> bool {
    env_flag_enabled("GATEWAY_ALLOW_UNAUTHENTICATED_INTERNAL_ROUTES")
}

pub fn unauthenticated_browser_executor_allowed() -> bool {
    env_flag_enabled("GATEWAY_ALLOW_UNAUTHENTICATED_BROWSER_EXECUTOR")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessBoundary {
    pub tenant_id: String,
    pub project_id: String,
}

impl AccessBoundary {
    pub fn new(tenant_id: &str, project_id: &str) -> Result<Self, GatewayError> {
        Ok(Self {
            tenant_id: required_identifier(tenant_id, "tenant_id")?,
            project_id: required_identifier(project_id, "project_id")?,
        })
    }

    pub fn ensure_same(&self, actual: &Self, subject: &str) -> Result<(), GatewayError> {
        if self == actual {
            return Ok(());
        }

        Err(GatewayError::bad_request(format!(
            "{subject} crosses the resolved tenant/project boundary"
        ))
        .with_code("access_boundary_mismatch"))
    }
}

pub fn ensure_allowed_resource_ids(
    requested_ids: &[String],
    allowed_ids: &[String],
    resource_label: &str,
) -> Result<Vec<String>, GatewayError> {
    let allowed = allowed_ids
        .iter()
        .filter_map(|value| normalized(Some(value.as_str())))
        .collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    let mut normalized_ids = Vec::new();
    let mut rejected = Vec::new();

    for requested_id in requested_ids {
        let Some(requested_id) = normalized(Some(requested_id.as_str())) else {
            continue;
        };
        if !seen.insert(requested_id) {
            continue;
        }
        if allowed.contains(requested_id) {
            normalized_ids.push(requested_id.to_string());
        } else {
            rejected.push(requested_id.to_string());
        }
    }

    if rejected.is_empty() {
        return Ok(normalized_ids);
    }

    Err(GatewayError::bad_request(format!(
        "{resource_label} IDs are outside the resolved tenant/project boundary: {}",
        rejected.join(", ")
    ))
    .with_code("access_boundary_mismatch"))
}

fn required_identifier(value: &str, label: &str) -> Result<String, GatewayError> {
    normalized(Some(value)).map(str::to_string).ok_or_else(|| {
        GatewayError::bad_request(format!("{label} must not be empty"))
            .with_code("access_boundary_invalid")
    })
}

pub(crate) fn management_request_token<'a>(
    bearer: Option<&'a str>,
    headers: &'a HeaderMap,
) -> Option<&'a str> {
    header_value(headers, "x-management-token")
        .or_else(|| header_value(headers, "x-internal-api-key"))
        .or_else(|| normalized(bearer))
        .or_else(|| bearer_from_authorization(headers))
}

fn header_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| normalized(Some(value)))
}

fn bearer_from_authorization(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .and_then(|value| normalized(Some(value)))
}

fn normalized(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn env_flag_enabled(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}
