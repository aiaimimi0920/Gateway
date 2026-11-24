//! Bind selected assets to one original account and workspace before creating a new audit row.
use super::{flow, invalid, store, GatewayError};
use crate::console::RouteConfigRuntime;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Binding {
    pub task_id: String,
    pub key: String,
    pub id: String,
}
pub(super) async fn bind(
    journal: &store::Store,
    target: &super::super::Input,
    request: &flow::Request,
    runtime: Option<&RouteConfigRuntime>,
) -> Result<Option<Binding>, GatewayError> {
    let Some(reuse) = &request.reuse else {
        return Ok(None);
    };
    // Reuse requires the same account proof as reads, including published OAuth rotations.
    let source = store::load(journal, &reuse.task_id, target, runtime).await?;
    let workspace = source.workspace_id.as_ref().ok_or_else(invalid)?;
    let selected = reuse.assets(request.kind).map_err(|_| invalid())?;
    for (id, kind) in selected {
        if !source.assets.iter().any(|asset| {
            asset["id"] == id && asset["kind"] == kind.name() && asset["state"] == "ready"
        }) {
            return Err(invalid());
        }
    }
    // Copy the verified root key once; later retention of ancestor audits is not needed to submit.
    let key = match &source.workspace_binding {
        Some(binding) if binding.id == *workspace => binding.key.clone(),
        Some(_) => return Err(invalid()),
        None => source.id.clone(),
    };
    Ok(Some(Binding {
        task_id: source.id,
        key,
        id: workspace.clone(),
    }))
}
