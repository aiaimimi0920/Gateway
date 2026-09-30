use crate::redis::keys;
#[cfg(test)]
use crate::routing::config::ProviderConfigYaml;
use crate::routing::config::ProviderCredentialYaml;
use serde::{Deserialize, Serialize};

mod claim;
mod completion;
mod creation;
mod delivery;
mod demand;
mod local;
mod local_delivery;
#[cfg(test)]
mod local_tests;
mod notifications;
mod storage;
mod task_clock;
mod validation;

use claim::{claim_conflict, require_claimed_task};
pub use claim::{claim_credential_refill_task, renew_credential_refill_task};
pub use completion::{complete_credential_refill_task, fail_credential_refill_task};
use creation::create_task_from_demand;
pub use creation::create_user_requested_refill_task;
#[cfg(test)]
use creation::hash_idempotency_key;
#[cfg(test)]
use delivery::append_refill_credentials;
use delivery::deliver_refill_result;
#[cfg(test)]
use demand::{active_credential_count, identity_category_deficit, provider_path_segment};
pub use demand::{demand_for_provider, list_credential_refill_demands};
pub use notifications::{
    publish_notification_refill_demands_once, start_credential_refill_notification_task,
};
pub use storage::list_credential_refill_tasks;
use storage::{
    load_outstanding_task, load_task, pending_task_ids, redis_connection, redis_operation_error,
    serialize_task,
};
use task_clock::{future_rfc3339, now_rfc3339};
use validation::{
    ensure_refill_enabled, normalize_identifier, normalize_optional_text, normalize_required_text,
    validate_folder_paths, validate_requested_count,
};

const MAX_REQUESTED_COUNT: usize = 10_000;
const MAX_TASK_LIST_LIMIT: usize = 200;
const MAX_WORKER_ID_LENGTH: usize = 128;
const MAX_IDEMPOTENCY_KEY_LENGTH: usize = 160;
const MAX_FAILURE_REASON_LENGTH: usize = 320;
const MAX_ARTIFACT_REFERENCE_LENGTH: usize = 2_048;
const MAX_FOLDER_PATHS: usize = 128;
const MAX_FOLDER_PATH_LENGTH: usize = 512;
const CREDENTIAL_REFILL_ROOT_API: &str = "/v1/internal/gateway/credential-pool-refill";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialRefillTrigger {
    Notification,
    Inquiry,
    UserRequested,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialRefillTaskState {
    Pending,
    Claimed,
    Succeeded,
    Failed,
}

impl CredentialRefillTaskState {
    fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialRefillDeliveryMode {
    FolderSync,
    GatewayPull,
    DirectCallback,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialRefillDemandView {
    pub provider_id: String,
    pub provider_label: String,
    pub target_size: usize,
    pub credential_count: usize,
    pub active_credential_count: usize,
    pub deficit: usize,
    pub needs_refill: bool,
    pub auto_refill_enabled: bool,
    pub direct_driver_configured: bool,
    pub notification_enabled: bool,
    pub inquiry_enabled: bool,
    pub user_request_enabled: bool,
    pub outstanding_task_id: Option<String>,
    pub outstanding_task_state: Option<CredentialRefillTaskState>,
    pub notification_api: String,
    pub inquiry_api: String,
    pub credential_storage_path: Option<String>,
    pub storage_password_configured: bool,
    pub archive_storage_path: Option<String>,
    pub archived_credential_count: usize,
    pub permanent_delete_enabled: bool,
    pub revision_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CredentialRefillTaskRecord {
    id: String,
    provider_id: String,
    provider_label: String,
    trigger: CredentialRefillTrigger,
    state: CredentialRefillTaskState,
    requested_count: usize,
    target_size: usize,
    active_credential_count: usize,
    route_revision: String,
    created_at: String,
    updated_at: String,
    #[serde(default)]
    worker_id: Option<String>,
    #[serde(default)]
    claim_token: Option<String>,
    #[serde(default)]
    lease_until: Option<String>,
    #[serde(default)]
    attempt: usize,
    #[serde(default)]
    delivery_mode: Option<CredentialRefillDeliveryMode>,
    #[serde(default)]
    created_count: usize,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    revision_id: Option<String>,
    #[serde(default)]
    idempotency_key: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialRefillTaskView {
    pub id: String,
    pub provider_id: String,
    pub provider_label: String,
    pub trigger: CredentialRefillTrigger,
    pub state: CredentialRefillTaskState,
    pub requested_count: usize,
    pub target_size: usize,
    pub active_credential_count: usize,
    pub route_revision: String,
    pub created_at: String,
    pub updated_at: String,
    pub worker_id: Option<String>,
    pub lease_until: Option<String>,
    pub attempt: usize,
    pub delivery_mode: Option<CredentialRefillDeliveryMode>,
    pub created_count: usize,
    pub message: Option<String>,
    pub revision_id: Option<String>,
}

impl From<&CredentialRefillTaskRecord> for CredentialRefillTaskView {
    fn from(record: &CredentialRefillTaskRecord) -> Self {
        Self {
            id: record.id.clone(),
            provider_id: record.provider_id.clone(),
            provider_label: record.provider_label.clone(),
            trigger: record.trigger,
            state: record.state,
            requested_count: record.requested_count,
            target_size: record.target_size,
            active_credential_count: record.active_credential_count,
            route_revision: record.route_revision.clone(),
            created_at: record.created_at.clone(),
            updated_at: record.updated_at.clone(),
            worker_id: record.worker_id.clone(),
            lease_until: record.lease_until.clone(),
            attempt: record.attempt,
            delivery_mode: record.delivery_mode,
            created_count: record.created_count,
            message: record.message.clone(),
            revision_id: record.revision_id.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Default)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct CreateCredentialRefillRequestInput {
    pub requested_count: Option<usize>,
    pub idempotency_key: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCredentialRefillTaskResult {
    pub task: CredentialRefillTaskView,
    pub created: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ClaimCredentialRefillTaskInput {
    pub worker_id: String,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub provider_ids: Vec<String>,
    #[serde(default)]
    pub lease_seconds: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimCredentialRefillTaskResult {
    pub task: CredentialRefillTaskView,
    pub claim_token: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RenewCredentialRefillTaskInput {
    pub claim_token: String,
    #[serde(default)]
    pub lease_seconds: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FailCredentialRefillTaskInput {
    pub claim_token: String,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CompleteCredentialRefillTaskInput {
    pub claim_token: String,
    pub delivery: CredentialRefillDeliveryInput,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum CredentialRefillDeliveryInput {
    FolderSync {
        #[serde(default, rename = "relativePaths")]
        relative_paths: Vec<String>,
    },
    GatewayPull {
        #[serde(rename = "artifactReference")]
        artifact_reference: String,
    },
    DirectCallback {
        credentials: Vec<ProviderCredentialYaml>,
    },
}

#[derive(Clone, Debug, Deserialize, Default)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct CredentialRefillTaskFilters {
    pub provider_id: Option<String>,
    pub state: Option<CredentialRefillTaskState>,
    pub limit: Option<usize>,
}

#[derive(Debug)]
struct DeliveryOutcome {
    mode: CredentialRefillDeliveryMode,
    created_count: usize,
    message: String,
    revision_id: Option<String>,
}

pub fn stream_key() -> &'static str {
    keys::credential_refill_stream_key()
}

#[cfg(test)]
mod tests;
