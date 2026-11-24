//! One bounded mutation/read, with checkpoints before generation and after receipt validation.
use super::{flow, review, store, Action, GatewayError};
use crate::protocol::luma_oauth::TokenMaterial;
use serde_json::json;
use tokio::time::Instant;

#[allow(clippy::too_many_arguments)]
pub(super) async fn operation(
    journal: &store::Store,
    mut task: store::Task,
    action: Action,
    request: Option<&flow::Request>,
    answer: Option<&review::Answer>,
    material: &TokenMaterial,
    endpoint: &str,
    permitted: &(dyn Fn() -> bool + Sync),
    deadline: Instant,
) -> Result<store::Task, GatewayError> {
    let http = rquest::Client::builder()
        .build()
        .map_err(|_| GatewayError::server_error("MCP client unavailable."))?;
    let result = tokio::time::timeout_at(
        deadline,
        step(
            journal, &mut task, action, request, answer, material, endpoint, &http, permitted,
        ),
    )
    .await;
    match result {
        Ok(Ok(())) => task.error = None,
        Ok(Err(error)) => task.error = Some(error.to_string()),
        Err(_) => task.error = Some("luma_mcp_timeout".into()),
    }
    review::interrupted(&mut task);
    if matches!(task.state.as_str(), "opening" | "submitting" | "responding") {
        task.state = "unconfirmed".into();
    }
    task.lease_until = 0;
    journal.save(&mut task, false).await?;
    Ok(task)
}
async fn checkpoint(journal: &store::Store, task: &mut store::Task) -> Result<(), flow::Error> {
    journal
        .save(task, false)
        .await
        .map_err(|_| flow::Error::Rejected)
}
#[allow(clippy::too_many_arguments)]
async fn step(
    journal: &store::Store,
    task: &mut store::Task,
    action: Action,
    request: Option<&flow::Request>,
    answer: Option<&review::Answer>,
    material: &TokenMaterial,
    endpoint: &str,
    http: &rquest::Client,
    permitted: &(dyn Fn() -> bool + Sync),
) -> Result<(), flow::Error> {
    if journal
        .expired(task)
        .await
        .map_err(|_| flow::Error::Rejected)?
        || !permitted()
    {
        return Err(flow::Error::Rejected);
    }
    if task.ended && action != Action::Read {
        return Ok(());
    }
    if action == Action::Start {
        let request = request.ok_or(flow::Error::InvalidResponse)?;
        let mut args = request.open(&task.id);
        if let Some(binding) = &task.workspace_binding {
            args["workspace_key"] = json!(binding.key);
        }
        let value = flow::call(http, endpoint, material, flow::Tool::Open, args).await?;
        let (session, workspace) = flow::session(&value)?;
        task.session_id = Some(session.clone());
        task.workspace_id = Some(workspace.clone());
        task.state = "submitting".into();
        checkpoint(journal, task).await?;
        // Persist the opened session for cleanup, but never render in a substituted workspace.
        if task
            .workspace_binding
            .as_ref()
            .is_some_and(|binding| binding.id != workspace)
        {
            return Err(flow::Error::InvalidResponse);
        }
        // An account edit or drain while opening must not dispatch a render with stale authorization.
        if !permitted() {
            return Err(flow::Error::Rejected);
        }
        let (tool, args) = request.generate(&task.id, &session)?;
        let value = flow::call(http, endpoint, material, tool, args).await?;
        task.run_id = Some(flow::run(&value)?);
        task.state = "running".into();
        checkpoint(journal, task).await?;
        return Ok(());
    }
    let Some(session) = task.session_id.clone() else {
        return Ok(());
    };
    if action == Action::Respond {
        review::respond(
            task,
            answer.ok_or(flow::Error::InvalidResponse)?,
            material,
            endpoint,
            http,
        )
        .await?;
        checkpoint(journal, task).await?;
        // Delivery is a separate fact from later session progress; a failed read never resends it.
        if !permitted() {
            return Err(flow::Error::Rejected);
        }
        review::follow(journal, task, material, endpoint, http).await?;
        return Ok(());
    }
    if action == Action::Cancel || action == Action::End {
        let run = if action == Action::Cancel {
            task.run_id.as_deref()
        } else {
            None
        };
        if action == Action::Cancel && run.is_none() {
            return Err(flow::Error::InvalidResponse);
        }
        let args = match run {
            Some(run) => json!({"run_id":run}),
            None => json!({"session_id":session,"end":true}),
        };
        let value = flow::call(http, endpoint, material, flow::Tool::Cancel, args).await?;
        let status = flow::cancellation(&value, &session, run)?;
        if status == "ended" {
            task.ended = true;
        } else {
            task.state = status;
        }
        task.review = None;
        checkpoint(journal, task).await?;
    } else if let Some(run) = task.run_id.clone().filter(|_| !task.ended) {
        review::follow(journal, task, material, endpoint, http).await?;
        if !permitted() {
            return Err(flow::Error::Rejected);
        }
        let value = flow::call(
            http,
            endpoint,
            material,
            flow::Tool::Await,
            json!({"run_id":run,"timeout_s":1}),
        )
        .await?;
        let progress = flow::progress(&value, &session, &run, task.kind)?;
        let state = progress["state"]
            .as_str()
            .ok_or(flow::Error::InvalidResponse)?;
        task.review = if matches!(state, "running" | "needs_input") {
            flow::select_review(&value["pending"], task.session_update.as_ref(), &run)?
        } else {
            None
        };
        task.state = if task.review.is_some() {
            "needs_input"
        } else {
            state
        }
        .into();
        // Preserve run state even if a subsequent asset or usage query fails.
        checkpoint(journal, task).await?;
        if let Some(review) = &mut task.review {
            for chunk in review.show.chunks(4) {
                let value = flow::call(
                    http,
                    endpoint,
                    material,
                    flow::Tool::View,
                    json!({"session_id":session,"asset_ids":chunk}),
                )
                .await?;
                review.assets.extend(flow::review_assets(&value, chunk)?);
            }
        }
        let ids: Vec<String> = serde_json::from_value(progress["assetIds"].clone())
            .map_err(|_| flow::Error::InvalidResponse)?;
        let mut assets = Vec::new();
        for chunk in ids.chunks(4) {
            let value = flow::call(
                http,
                endpoint,
                material,
                flow::Tool::View,
                json!({"session_id":session,"asset_ids":chunk}),
            )
            .await?;
            assets.extend(flow::assets(&value, chunk, task.kind)?);
        }
        task.assets = assets;
    }
    let value = flow::call(
        http,
        endpoint,
        material,
        flow::Tool::Usage,
        json!({"session_id":session}),
    )
    .await?;
    let mut usage = flow::usage(&value, &session)?;
    // Even a currently resolved rollup can change while this run is active.
    if !matches!(task.state.as_str(), "completed" | "failed" | "cancelled") && !task.ended {
        usage["settled"] = json!(false);
    }
    task.usage = Some(usage);
    Ok(())
}
