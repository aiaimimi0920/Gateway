//! One text child at a time, no waiting queue and no detached generation/cleanup task.
use super::*;
use crate::console::copilot_catalog::Settings;
use gateway_copilot_runtime::{CopilotRuntime, RuntimeConfig, TextError};
use std::time::Duration;
use tokio::sync::{Semaphore, SemaphorePermit};

pub(super) static SLOT: Semaphore = Semaphore::const_new(1);

struct Lease<'a> {
    permit: Option<SemaphorePermit<'a>>,
    unconfirmed: bool,
}

impl Drop for Lease<'_> {
    fn drop(&mut self) {
        if self.unconfirmed {
            // force_stop is not an exit receipt. Quarantine this text slot until process restart.
            if let Some(permit) = self.permit.take() {
                permit.forget();
            }
        }
    }
}

pub(super) async fn run(
    identity: &CopilotIdentity<'_>,
    gate: &ProviderAttemptGate,
    settings: Settings,
    slot: &Semaphore,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let mut lease = Lease {
        permit: Some(slot.try_acquire().map_err(|_| unavailable())?),
        unconfirmed: false,
    };
    if !settings.executable.is_absolute() || !settings.workspace_root.is_absolute() {
        return Err(unavailable());
    }
    let root = tokio::fs::canonicalize(&settings.workspace_root)
        .await
        .map_err(|_| unavailable())?;
    if !root.is_dir() || !settings.executable.is_file() {
        return Err(unavailable());
    }
    gate.recheck_inflight_eligibility().await.map_err(abort)?;
    identity.token_for_send().await?;
    let directory = root.join(format!("copilot-text-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir(&directory)
        .await
        .map_err(|_| unavailable())?;
    let home = directory.join("home");
    tokio::fs::create_dir(&home)
        .await
        .map_err(|_| unavailable())?;
    // Arm BEFORE polling startup: startup can spawn a child before returning an owner.
    lease.unconfirmed = true;
    let owner = tokio::time::timeout(
        Duration::from_secs(15),
        CopilotRuntime::start(RuntimeConfig {
            executable: settings.executable,
            directory: home,
        }),
    )
    .await
    .map_err(|_| unavailable())?
    .map_err(|_| unavailable())?;
    let authorized = async {
        gate.recheck_inflight_eligibility().await.map_err(abort)?;
        identity.token_for_send().await
    }
    .await;
    let result = match authorized {
        Ok(token) => {
            // Includes session creation, generation and stop; the outer request budget may be shorter.
            let result = tokio::time::timeout(
                Duration::from_secs(160),
                owner.complete_text(
                    token.to_owned(),
                    identity.request().model.to_owned(),
                    identity.request().prompt.to_owned(),
                    Duration::from_secs(120),
                ),
            )
            .await
            .map_err(|_| unavailable())?;
            if matches!(&result, Err(TextError::Shutdown(_))) {
                return Err(unavailable());
            }
            // complete_text only returns other errors after a confirmed stop.
            lease.unconfirmed = false;
            result.map_err(|_| unavailable())
        }
        Err(error) => {
            tokio::time::timeout(Duration::from_secs(40), owner.stop())
                .await
                .map_err(|_| unavailable())?
                .map_err(|_| unavailable())?;
            lease.unconfirmed = false;
            Err(error)
        }
    };
    // Only this freshly-created directory, and only after confirmed stop. Keep uncertain exits intact.
    tokio::fs::remove_dir_all(&directory)
        .await
        .map_err(|_| unavailable())?;
    let completion = result?;
    gate.recheck_inflight_eligibility().await.map_err(abort)?;
    identity.token_for_send().await?;
    Ok(CanonicalRelayResponse {
        bedrock_turn: None,
        vertex_turn: None,
        model_declaration: None,
        // No SDK model declaration or HTTP status was observed. Public model projection is separate.
        model: String::new(),
        text: completion.text,
        sources: vec![],
        usage: None,
        tool_calls: vec![],
        upstream_status: None,
        finish_reason: None,
    })
}

#[cfg(test)]
mod tests;
