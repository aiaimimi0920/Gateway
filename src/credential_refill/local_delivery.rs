//! Prepared delivery survives a crash between route commit and task completion.
use super::*;
use crate::{error::GatewayError, local_runtime::storage_error, state::AppState};
use std::sync::Arc;

pub(super) async fn deliver(
    state: &Arc<AppState>,
    task: &CredentialRefillTaskRecord,
    input: CredentialRefillDeliveryInput,
) -> Result<DeliveryOutcome, GatewayError> {
    let db = state.local_runtime.as_ref().expect("local delivery owner");
    let prepared: Option<(String, String, i64)> = sqlx::query_as(
        "SELECT payload, mode, expected_count FROM refill_deliveries WHERE task_id = ?",
    )
    .bind(&task.id)
    .fetch_optional(&db.pool)
    .await
    .map_err(storage_error)?;
    let (delivery, mode, expected_count) = if let Some((payload, mode, count)) = prepared {
        let delivery = serde_json::from_str(&payload)
            .map_err(|_| GatewayError::server_error("Prepared refill delivery is corrupt"))?;
        let mode = serde_json::from_str(&mode)
            .map_err(|_| GatewayError::server_error("Prepared refill delivery mode is corrupt"))?;
        (delivery, mode, count as usize)
    } else {
        let (delivery, mode) = match input {
            CredentialRefillDeliveryInput::GatewayPull { artifact_reference } => {
                let reference = normalize_required_text(
                    &artifact_reference,
                    "artifactReference",
                    MAX_ARTIFACT_REFERENCE_LENGTH,
                )?;
                let credentials =
                    crate::credential_pool_automation::collect_refill_credentials_from_driver(
                        state,
                        &task.provider_id,
                        &task.id,
                        task.requested_count,
                        &reference,
                    )
                    .await
                    .map_err(|_| {
                        GatewayError::bad_request("Refill driver could not provide credentials")
                            .with_code("credential_refill_driver_pull_failed")
                    })?;
                (
                    CredentialRefillDeliveryInput::DirectCallback { credentials },
                    CredentialRefillDeliveryMode::GatewayPull,
                )
            }
            value @ CredentialRefillDeliveryInput::DirectCallback { .. } => {
                (value, CredentialRefillDeliveryMode::DirectCallback)
            }
            CredentialRefillDeliveryInput::FolderSync { relative_paths } => {
                let credentials = folder_delivery::collect(state, task, relative_paths).await?;
                (
                    CredentialRefillDeliveryInput::DirectCallback { credentials },
                    CredentialRefillDeliveryMode::FolderSync,
                )
            }
        };
        let expected = if let CredentialRefillDeliveryInput::DirectCallback { credentials } =
            &delivery
        {
            if credentials.is_empty() || credentials.len() > task.requested_count {
                return Err(GatewayError::bad_request(
                    "Refill credential count is outside the requested bounds",
                ));
            }
            let snapshot = state.route_config.snapshot();
            let mut provider = snapshot
                .document()
                .providers
                .iter()
                .find(|provider| provider.id == task.provider_id)
                .cloned()
                .ok_or_else(|| GatewayError::not_found("Refill provider does not exist"))?;
            let availability = crate::credential_pool_automation::availability::pool_availability(
                state, &provider,
            )
            .await?;
            delivery::append_refill_credentials_with_capacity(
                &mut provider,
                credentials.clone(),
                availability.remaining,
            )?
        } else {
            0
        };
        let payload = serde_json::to_string(&delivery)
            .map_err(|_| GatewayError::server_error("Cannot prepare refill delivery"))?;
        let mode_json = serde_json::to_string(&mode)
            .map_err(|_| GatewayError::server_error("Cannot serialize refill delivery mode"))?;
        sqlx::query("INSERT INTO refill_deliveries(task_id, payload, mode, expected_count) VALUES (?, ?, ?, ?)")
            .bind(&task.id).bind(payload).bind(mode_json).bind(expected as i64)
            .execute(&db.pool).await.map_err(storage_error)?;
        (delivery, mode, expected)
    };
    let mut outcome = deliver_refill_result(state, task, delivery).await?;
    outcome.mode = mode;
    outcome.created_count = expected_count;
    outcome.message = format!("Credential delivery committed: {expected_count}");
    Ok(outcome)
}
