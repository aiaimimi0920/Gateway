//! Select the handshake contract once; never downgrade after an upstream rejection.
use super::super::{common::ResponseStage, web_reverse::ChatGptWebRequestContext};
use super::policy::{conversation_path, extra_body_string};
use crate::routing::candidate::ProviderAccountPayload;
use crate::{error::GatewayError, protocol::chatgpt::web_reverse as surface};
use rquest::Client;
use serde_json::json;
use std::{collections::HashMap, time::Duration};

mod challenges;
mod exchange;
mod solver;

pub(super) async fn get_requirements(
    http: &Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    request_context: &ChatGptWebRequestContext,
    bootstrap: &surface::ChatGptWebBootstrap,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<surface::ChatRequirements, GatewayError> {
    let two_stage = uses_two_stage(payload)?;
    let source = solver::legacy(bootstrap, &request_context.user_agent).await?;
    let exchange = exchange::Exchange {
        http,
        timeout,
        payload,
        request_context,
        extra_headers,
    };
    let path = if two_stage {
        surface::CHATGPT_WEB_REQUIREMENTS_PREPARE_PATH
    } else {
        surface::CHATGPT_WEB_DEFAULT_REQUIREMENTS_PATH
    };
    let stage = if two_stage {
        ResponseStage::RequirementsPrepare
    } else {
        ResponseStage::Requirements
    };
    let data = exchange.send(path, &json!({"p": source}), stage).await?;
    // A missing prepare token is a protocol error, not a reason to retry the legacy endpoint.
    let tokens =
        challenges::resolve(&data, &source, bootstrap, &request_context.user_agent).await?;
    let token_name = if two_stage { "prepare_token" } else { "token" };
    let prepared_token = exchange::required_token(&data, token_name)?;
    let data = if two_stage {
        exchange
            .send(
                surface::CHATGPT_WEB_REQUIREMENTS_FINALIZE_PATH,
                &json!({
                    "prepare_token": prepared_token,
                    "proof_token": tokens.proof_token.as_deref().unwrap_or_default(),
                    "turnstile_token": tokens.turnstile_token.as_deref().unwrap_or_default()
                }),
                ResponseStage::RequirementsFinalize,
            )
            .await?
    } else {
        data
    };
    Ok(surface::ChatRequirements {
        token: exchange::required_token(&data, "token")?,
        proof_token: tokens.proof_token,
        turnstile_token: tokens.turnstile_token,
        so_token: exchange::optional_token(&data, "so_token")?,
    })
}

fn uses_two_stage(payload: &ProviderAccountPayload) -> Result<bool, GatewayError> {
    match extra_body_string(
        payload,
        &["chatgptWebRequirementsMode", "chatgptRequirementsMode"],
    )
    .as_deref()
    {
        Some("prepare_finalize") => Ok(true),
        Some("legacy") => Ok(false),
        Some(_) => Err(GatewayError::bad_request(
            "ChatGPT Web requirements mode must be prepare_finalize or legacy.",
        )
        .with_code("chatgpt_web_requirements_mode_invalid")),
        None => {
            Ok(conversation_path(payload, false)
                != surface::CHATGPT_WEB_DEFAULT_F_CONVERSATION_PATH)
        }
    }
}
