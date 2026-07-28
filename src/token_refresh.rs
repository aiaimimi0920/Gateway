// ---------------------------------------------------------------------------
// token_refresh.rs — Background OAuth token auto-refresh
//
// Periodically checks all provider credentials for expiring OAuth tokens
// and refreshes them before they expire. Supports any OAuth2 provider
// that uses the refresh_token grant (Qwen, Codex, etc.).
// ---------------------------------------------------------------------------

use std::sync::Arc;
use std::time::Duration;

use rquest::Client;
use serde::Deserialize;

use crate::redis::credential_cache;
use crate::routing::config::{
    effective_refresh_lifetime_secs, future_rfc3339_after_secs, RouteConfigStore,
};

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
}

/// Refresh an OAuth access_token using the refresh_token grant.
async fn refresh_oauth_token(
    http: &Client,
    endpoint: &str,
    refresh_token: &str,
    client_id: &str,
) -> Result<(String, Option<String>, u64), String> {
    let params = [
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", client_id),
    ];

    let resp = http
        .post(endpoint)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&params)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("refresh request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!(
            "refresh failed ({status}): {}",
            &body[..body.len().min(200)]
        ));
    }

    let body: TokenResponse = resp
        .json()
        .await
        .map_err(|e| format!("parse refresh response: {e}"))?;

    Ok((
        body.access_token,
        body.refresh_token,
        body.expires_in.unwrap_or(21600),
    ))
}

/// Background task: checks all credentials every `check_interval_secs` and
/// refreshes tokens that expire within `refresh_margin_secs`.
pub async fn start_token_refresh_task(
    route_config: Arc<RouteConfigStore>,
    http: Client,
    redis_pool: deadpool_redis::Pool,
    check_interval_secs: u64,
    refresh_margin_secs: u64,
) {
    let check_interval = Duration::from_secs(check_interval_secs);
    let refresh_margin = Duration::from_secs(refresh_margin_secs);

    tracing::info!(
        check_interval_secs,
        refresh_margin_secs,
        "OAuth token refresh task started"
    );

    loop {
        tokio::time::sleep(check_interval).await;

        let providers = route_config.get_providers();

        for provider in &providers {
            for cred in &provider.credential_pool {
                if !cred.enabled {
                    continue;
                }
                let config = match &cred.refresh_config {
                    Some(c) => c,
                    None => continue,
                };
                let Some((attempt_generation, current_rt)) =
                    config.refresh_snapshot(refresh_margin)
                else {
                    continue;
                };

                match refresh_oauth_token(
                    &http,
                    &config.refresh_endpoint,
                    &current_rt,
                    &config.client_id,
                )
                .await
                {
                    Ok((new_access, new_refresh, expires_in)) => {
                        let expires_in = effective_refresh_lifetime_secs(expires_in);
                        let expires_at_iso = future_rfc3339_after_secs(expires_in);
                        let Some(applied_generation) = config.apply_refresh_if_active(
                            attempt_generation,
                            new_access.clone(),
                            new_refresh.clone(),
                            expires_in,
                            expires_at_iso.clone(),
                        ) else {
                            tracing::debug!(
                                cred_id = %cred.id,
                                provider = %provider.id,
                                "discarded OAuth refresh response for retired route snapshot"
                            );
                            continue;
                        };

                        // Write back to Redis so user-hosted credentials persist
                        // across cache reloads. Best-effort — don't block the loop.
                        if !config.is_generation_active(applied_generation) {
                            continue;
                        }
                        if let Err(e) = credential_cache::write_back_refreshed_token(
                            &redis_pool,
                            &cred.id,
                            &new_access,
                            new_refresh.as_deref(),
                        )
                        .await
                        {
                            tracing::debug!(
                                cred_id = %cred.id,
                                error = %e,
                                "token refresh Redis writeback failed (non-fatal)"
                            );
                        }

                        if !config.is_generation_active(applied_generation) {
                            tracing::debug!(
                                cred_id = %cred.id,
                                provider = %provider.id,
                                "stopped stale OAuth refresh writeback after route replacement"
                            );
                            continue;
                        }

                        if cred.payload.session_auth.is_some() || cred.payload.keepalive.is_some() {
                            let mut session_auth = cred.payload.session_auth.clone();
                            if let Some(ref mut session) = session_auth {
                                session.expires_at = Some(expires_at_iso.clone());
                            }
                            if let Err(e) = credential_cache::write_back_runtime_material(
                                &redis_pool,
                                &cred.id,
                                Some(&new_access),
                                None,
                                None,
                                session_auth.as_ref(),
                                cred.payload.keepalive.as_ref(),
                                Some(&expires_at_iso),
                                None,
                            )
                            .await
                            {
                                tracing::debug!(
                                    cred_id = %cred.id,
                                    error = %e,
                                    "token refresh runtime material writeback failed (non-fatal)"
                                );
                            }
                        }

                        tracing::info!(
                            cred_id = %cred.id,
                            provider = %provider.id,
                            expires_in_secs = expires_in,
                            "OAuth token refreshed"
                        );
                    }
                    Err(e) => {
                        tracing::error!(
                            cred_id = %cred.id,
                            provider = %provider.id,
                            error = %e,
                            "OAuth token refresh failed"
                        );
                    }
                }
            }
        }
    }
}
