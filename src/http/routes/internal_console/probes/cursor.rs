//! A manual probe reports configured Cursor accounts without creating an executable
//! target, inspecting their token, or changing health and routing evidence.
use super::{
    credential_probe_checked_at, select_requested, CredentialProbeResponse, ProviderProbeResponse,
};
use crate::{
    error::GatewayError,
    provider_runtime::{ConsoleProbeRequest, ProviderPayloadProbeStatus},
    routing::config::{cursor_remote, RouteConfigSnapshot},
};

fn not_executed(
    provider: &str,
    id: &str,
    enabled: bool,
    checked_at: &str,
) -> CredentialProbeResponse {
    let state = if enabled { "已启用" } else { "已暂停" };
    CredentialProbeResponse {
        provider_id: provider.into(),
        credential_id: id.into(),
        checked_at: checked_at.into(),
        status: ProviderPayloadProbeStatus::Unsupported,
        probe_point: "未执行".into(),
        message: format!("Cursor 账号{state}。不支持通用探测，未向 Cursor 发送请求。"),
        assessment: None,
        health_recovery: None,
    }
}

pub(super) fn account(snapshot: &RouteConfigSnapshot, id: &str) -> Option<CredentialProbeResponse> {
    snapshot
        .document()
        .providers
        .iter()
        .filter(|p| cursor_remote::is_provider(p))
        .find_map(|p| {
            let c = p.credentials.iter().find(|c| c.id.as_deref() == Some(id))?;
            Some(not_executed(
                &p.id,
                id,
                c.enabled == Some(true),
                &credential_probe_checked_at(),
            ))
        })
}

pub(super) fn pool(
    snapshot: &RouteConfigSnapshot,
    id: &str,
    options: Option<&ConsoleProbeRequest>,
) -> Result<Option<ProviderProbeResponse>, GatewayError> {
    let Some(provider) = snapshot
        .document()
        .providers
        .iter()
        .find(|p| p.id == id && cursor_remote::is_provider(p))
    else {
        return Ok(None);
    };
    // Validated Cursor records always have explicit IDs and cannot configure subpools or named plans.
    let mut accounts: Vec<_> = provider
        .credentials
        .iter()
        .map(|c| {
            (
                c.id.as_deref().expect("validated Cursor account ID"),
                c.enabled == Some(true),
            )
        })
        .collect();
    select_requested(&mut accounts, options, |t| t.0, |_, _| false)?;
    let checked_at = credential_probe_checked_at();
    let results: Vec<_> = accounts
        .into_iter()
        .map(|(account, enabled)| not_executed(id, account, enabled, &checked_at))
        .collect();
    Ok(Some(ProviderProbeResponse {
        provider_id: id.into(),
        status: ProviderPayloadProbeStatus::Unsupported,
        message: if results.is_empty() {
            "Cursor 服务商没有账号，未发送请求。"
        } else {
            "Cursor 账号不支持通用探测，未向 Cursor 发送请求。"
        }
        .into(),
        checked_at,
        total_count: results.len(),
        passed_count: 0,
        failed_count: 0,
        unsupported_count: results.len(),
        results,
    }))
}
