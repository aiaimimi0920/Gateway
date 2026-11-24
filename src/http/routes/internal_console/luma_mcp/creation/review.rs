//! Review admission and delivery receipts share the task's durable CAS lease.
use super::{flow, invalid, store, GatewayError};
use crate::protocol::luma_oauth::TokenMaterial;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Answer {
    invocation_id: String,
    fingerprint: String,
    result: Value,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Receipt {
    invocation_id: String,
    fingerprint: String,
    result_hash: String,
    idempotency_key: String,
    status: String,
}
pub(super) fn admit(
    task: &mut store::Task,
    answer: &Answer,
    confirmed: bool,
) -> Result<(), GatewayError> {
    let review = task.review.as_ref().ok_or_else(invalid)?;
    if !confirmed
        || task.ended
        || task.state != "needs_input"
        || !review.answerable()
        || review.invocation_id.as_deref() != Some(answer.invocation_id.as_str())
        || review.fingerprint != answer.fingerprint
        || task
            .session_update
            .as_ref()
            .is_some_and(|v| v["has_more"] == true || v["status"] == "ended")
        || answer.result.to_string().len() > 16 * 1024
        || !review.accepts_answer(&answer.result)
    {
        return Err(invalid());
    }
    if task.responses.len() >= 32
        || task
            .responses
            .iter()
            .any(|r| r.invocation_id == answer.invocation_id)
    {
        return Err(GatewayError::conflict(
            "This review was already submitted; read the existing task.",
        ));
    }
    task.responses.push(Receipt {
        invocation_id: answer.invocation_id.clone(),
        fingerprint: answer.fingerprint.clone(),
        result_hash: flow::digest(&answer.result),
        idempotency_key: format!("{}-answer-{}", task.id, task.responses.len()),
        status: "submitting".into(),
    });
    task.state = "responding".into();
    Ok(())
}
pub(super) fn interrupted(task: &mut store::Task) {
    for receipt in &mut task.responses {
        if receipt.status == "submitting" {
            receipt.status = "unconfirmed".into();
        }
    }
}
pub(super) async fn respond(
    task: &mut store::Task,
    answer: &Answer,
    material: &TokenMaterial,
    endpoint: &str,
    http: &rquest::Client,
) -> Result<(), flow::Error> {
    let session = task
        .session_id
        .as_deref()
        .ok_or(flow::Error::InvalidResponse)?;
    let run = task.run_id.as_deref().ok_or(flow::Error::InvalidResponse)?;
    let receipt = task.responses.last().ok_or(flow::Error::InvalidResponse)?;
    let value = flow::call(http, endpoint, material, flow::Tool::Respond, json!({
        "run_id":run,"invocation_id":answer.invocation_id,"result":answer.result,"idempotency_key":receipt.idempotency_key
    })).await?;
    let seq = flow::delivered(&value, session, run)?;
    task.responses
        .last_mut()
        .ok_or(flow::Error::InvalidResponse)?
        .status = "delivered".into();
    task.after_seq = Some(seq);
    task.session_update = None;
    task.review = None;
    task.state = "running".into();
    Ok(())
}
pub(super) async fn follow(
    journal: &store::Store,
    task: &mut store::Task,
    material: &TokenMaterial,
    endpoint: &str,
    http: &rquest::Client,
) -> Result<(), flow::Error> {
    let Some(after) = task.after_seq else {
        return Ok(());
    };
    let session = task
        .session_id
        .as_deref()
        .ok_or(flow::Error::InvalidResponse)?;
    let value = flow::call(
        http,
        endpoint,
        material,
        flow::Tool::SessionAwait,
        json!({"session_id":session,"after_seq":after,"timeout_s":1}),
    )
    .await?;
    task.after_seq = Some(flow::session_update(&value, session, after)?);
    // Keep this page through a later run/view failure; the next explicit read fetches the next page.
    // An ended session can still have unread pages and a final run result. Only explicit End
    // stops run polling; the reported session status separately blocks new answers.
    task.session_update = Some(value);
    // A newly observed page invalidates last read's answer form even if the run poll fails.
    task.review = None;
    journal
        .save(task, false)
        .await
        .map_err(|_| flow::Error::Rejected)
}
