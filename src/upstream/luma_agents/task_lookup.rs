//! Explicit read of one existing task. No generation, asset download, polling or ledger mutation.
use crate::{error::GatewayError, routing::candidate::ProviderAccountPayload};
use serde_json::Value;

pub(crate) fn task_id(value: &str) -> Result<String, GatewayError> {
    if value.len() == 36 {
        if let Ok(id) = uuid::Uuid::parse_str(value) {
            return Ok(id.to_string());
        }
    }
    Err(
        GatewayError::bad_request("Enter a complete Luma generation UUID.")
            .with_code("luma_task_id"),
    )
}

pub(crate) async fn execute(
    client: &rquest::Client,
    payload: &ProviderAccountPayload,
    id: &str,
) -> Result<Value, GatewayError> {
    let id = task_id(id)?;
    #[cfg(not(feature = "line-lumalabs-agents-official-api"))]
    {
        let _ = (client, payload, id);
        Err(
            GatewayError::service_unavailable("Luma Agents official API is not compiled.")
                .with_code("implementation_line_disabled"),
        )
    }
    #[cfg(feature = "line-lumalabs-agents-official-api")]
    {
        use super::{
            generation,
            transport::{self, Material},
        };
        use serde_json::json;
        use std::time::Duration;
        let material = Material::new(payload)?;
        let budget = Duration::from_secs(30);
        let mut total = 0;
        let job = tokio::time::timeout(
            budget,
            transport::json(client, &material, Some(&id), None, budget, &mut total),
        )
        .await
        .map_err(|_| transport::timeout_error())??;
        let invalid = || {
            GatewayError::server_error("Luma task metadata is invalid; upstream body omitted.")
                .with_code("luma_task_metadata")
        };
        let model = job["model"]
            .as_str()
            .filter(|value| {
                !value.is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
            })
            .ok_or_else(invalid)?;
        let kind = job["type"]
            .as_str()
            .filter(|value| {
                matches!(
                    *value,
                    "image" | "image_edit" | "video" | "video_edit" | "video_reframe" | "layering"
                )
            })
            .ok_or_else(invalid)?;
        let state = generation::validate_identity(&job, model, kind, &id)?;
        // Supplier completion does not establish local media verification or local payment.
        Ok(
            json!({"id":id,"model":model,"type":kind,"state":state,"usage":generation::usage(&job)?}),
        )
    }
}
