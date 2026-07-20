use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap as AxumHeaderMap;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::conversation_dataset::{build_clean_dataset_row, deterministic_sample_indices};
use crate::db;
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::object_storage::{
    build_gateway_conversation_archive_export_dataset_object_key,
    build_gateway_conversation_dataset_export_object_key, gateway_object_storage,
};
use crate::state::AppState;

use super::internal_gateway::assert_management_access;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationArchiveQuery {
    pub project_id: Option<String>,
    pub user_id: Option<String>,
    pub provider_account_id: Option<String>,
    pub provider_credential_ref: Option<String>,
    pub protocol_family: Option<String>,
    pub protocol_profile: Option<String>,
    pub endpoint_kind: Option<String>,
    pub requested_model: Option<String>,
    pub resolved_model: Option<String>,
    pub status: Option<String>,
    pub failure_class: Option<String>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationDatasetCreateBody {
    #[serde(flatten)]
    pub filters: ConversationArchiveQuery,
    pub sample_size: Option<usize>,
    pub created_by: Option<String>,
}

pub async fn list_conversation_archives(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Query(query): Query<ConversationArchiveQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let pg_pool = state
        .pg_pool
        .as_ref()
        .ok_or_else(|| GatewayError::service_unavailable("database is not configured"))?;
    let archives = db::list_conversation_archives(pg_pool, query.into_filters()).await?;
    Ok(Json(json!({ "archives": archives })))
}

pub async fn get_conversation_archive(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(archive_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let pg_pool = state
        .pg_pool
        .as_ref()
        .ok_or_else(|| GatewayError::service_unavailable("database is not configured"))?;
    let archive = db::get_conversation_archive(pg_pool, archive_id.trim())
        .await?
        .ok_or_else(|| GatewayError::not_found("conversation archive not found"))?;
    Ok(Json(json!({ "archive": archive })))
}

pub async fn get_conversation_archive_artifacts(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(archive_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let pg_pool = state
        .pg_pool
        .as_ref()
        .ok_or_else(|| GatewayError::service_unavailable("database is not configured"))?;
    let archive = db::get_conversation_archive(pg_pool, archive_id.trim())
        .await?
        .ok_or_else(|| GatewayError::not_found("conversation archive not found"))?;

    let request_artifact = match archive.request_object_key.as_deref() {
        Some(object_key) => read_optional_artifact(object_key).await,
        None => None,
    };
    let response_artifact = match archive.response_object_key.as_deref() {
        Some(object_key) => read_optional_artifact(object_key).await,
        None => None,
    };

    Ok(Json(json!({
        "archive": archive,
        "artifacts": {
            "requestArtifact": request_artifact,
            "responseArtifact": response_artifact
        }
    })))
}

pub async fn export_conversation_archives(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Json(body): Json<ConversationArchiveQuery>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let pg_pool = state
        .pg_pool
        .as_ref()
        .ok_or_else(|| GatewayError::service_unavailable("database is not configured"))?;
    let (export_id, archives) =
        db::export_conversation_archives(pg_pool, body.into_filters()).await?;
    let dataset_object_key =
        build_gateway_conversation_archive_export_dataset_object_key(&export_id);
    let mut lines = Vec::with_capacity(archives.len());
    for archive in &archives {
        let request_artifact = match archive.request_object_key.as_deref() {
            Some(object_key) => read_optional_artifact(object_key).await,
            None => None,
        };
        let response_artifact = match archive.response_object_key.as_deref() {
            Some(object_key) => read_optional_artifact(object_key).await,
            None => None,
        };
        lines.push(
            serde_json::to_string(&json!({
                "archiveId": archive.id,
                "requestId": archive.request_id,
                "projectId": archive.project_id,
                "userId": archive.user_id,
                "providerAccountId": archive.provider_account_id,
                "providerCredentialRef": archive.provider_credential_ref,
                "protocolFamily": archive.protocol_family,
                "protocolProfile": archive.protocol_profile,
                "endpointKind": archive.endpoint_kind,
                "requestedModel": archive.requested_model,
                "resolvedModel": archive.resolved_model,
                "status": archive.status,
                "failureClass": archive.failure_class,
                "failureScope": archive.failure_scope,
                "request": request_artifact,
                "response": response_artifact,
                "createdAt": archive.created_at,
            }))
            .map_err(|error| {
                GatewayError::server_error(format!("serialize archive export row: {error}"))
            })?,
        );
    }
    let dataset = if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    };
    gateway_object_storage()?
        .put_bytes(
            &dataset_object_key,
            dataset.into_bytes(),
            "application/x-ndjson",
        )
        .await?;

    let export = db::GatewayConversationArchiveExportView {
        export_id,
        dataset_object_key,
        row_count: archives.len(),
        created_at: OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|_| OffsetDateTime::now_utc().unix_timestamp().to_string()),
    };

    Ok(Json(json!({ "export": export })))
}

pub async fn create_conversation_dataset_export(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Json(body): Json<ConversationDatasetCreateBody>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let pg_pool = state
        .pg_pool
        .as_ref()
        .ok_or_else(|| GatewayError::service_unavailable("database is not configured"))?;
    let filter_value = serde_json::to_value(&body.filters).map_err(|error| {
        GatewayError::server_error(format!("serialize dataset filter: {error}"))
    })?;
    let archives = db::list_conversation_archives(pg_pool, body.filters.into_filters()).await?;
    let sample_size = body
        .sample_size
        .unwrap_or(archives.len())
        .min(archives.len());
    let indices = deterministic_sample_indices(archives.len(), sample_size);

    let mut lines = Vec::with_capacity(indices.len());
    for index in indices {
        let archive = archives
            .get(index)
            .ok_or_else(|| GatewayError::server_error("sample index out of range"))?;
        let request_artifact = match archive.request_object_key.as_deref() {
            Some(object_key) => read_optional_artifact(object_key).await,
            None => None,
        };
        let response_artifact = match archive.response_object_key.as_deref() {
            Some(object_key) => read_optional_artifact(object_key).await,
            None => None,
        };
        let row = build_clean_dataset_row(archive, request_artifact, response_artifact);
        lines.push(serde_json::to_string(&row).map_err(|error| {
            GatewayError::server_error(format!("serialize dataset row: {error}"))
        })?);
    }

    let dataset_id = Uuid::new_v4().to_string();
    let dataset_object_key =
        build_gateway_conversation_dataset_export_object_key(&dataset_id, "dataset.jsonl");
    let manifest_object_key =
        build_gateway_conversation_dataset_export_object_key(&dataset_id, "manifest.json");
    let dataset = if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    };
    let manifest = json!({
        "datasetId": dataset_id,
        "rowCount": lines.len(),
        "sampleSize": sample_size,
        "filter": filter_value.clone(),
        "status": "review_pending",
        "publishGate": "manual_review_required",
        "createdAt": OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|_| OffsetDateTime::now_utc().unix_timestamp().to_string()),
    });

    let storage = gateway_object_storage()?;
    storage
        .put_bytes(
            &dataset_object_key,
            dataset.into_bytes(),
            "application/x-ndjson",
        )
        .await?;
    storage
        .put_bytes(
            &manifest_object_key,
            serde_json::to_vec_pretty(&manifest).map_err(|error| {
                GatewayError::server_error(format!("serialize dataset manifest: {error}"))
            })?,
            "application/json",
        )
        .await?;

    let dataset_export = db::create_conversation_dataset_export(
        pg_pool,
        db::CreateConversationDatasetExportInput {
            id: dataset_id,
            filter: filter_value,
            sample_size: body
                .sample_size
                .map(|value| value.min(i32::MAX as usize) as i32),
            row_count: lines.len().min(i32::MAX as usize) as i32,
            dataset_object_key,
            manifest_object_key,
            created_by: body.created_by,
        },
    )
    .await?;

    Ok(Json(json!({ "dataset": dataset_export })))
}

pub async fn list_conversation_dataset_exports(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let pg_pool = state
        .pg_pool
        .as_ref()
        .ok_or_else(|| GatewayError::service_unavailable("database is not configured"))?;
    let datasets = db::list_conversation_dataset_exports(pg_pool).await?;
    Ok(Json(json!({ "datasets": datasets })))
}

pub async fn review_conversation_dataset_export(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(dataset_id): Path<String>,
    Json(body): Json<db::ReviewConversationDatasetExportInput>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let pg_pool = state
        .pg_pool
        .as_ref()
        .ok_or_else(|| GatewayError::service_unavailable("database is not configured"))?;
    let dataset = db::review_conversation_dataset_export(pg_pool, dataset_id.trim(), body).await?;
    Ok(Json(json!({ "dataset": dataset })))
}

pub async fn publish_conversation_dataset_export(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: AxumHeaderMap,
    Path(dataset_id): Path<String>,
) -> Result<Json<Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    let pg_pool = state
        .pg_pool
        .as_ref()
        .ok_or_else(|| GatewayError::service_unavailable("database is not configured"))?;
    let dataset = db::publish_conversation_dataset_export(pg_pool, dataset_id.trim()).await?;
    Ok(Json(json!({ "dataset": dataset })))
}

impl ConversationArchiveQuery {
    fn into_filters(self) -> db::ConversationArchiveFilters {
        db::ConversationArchiveFilters {
            project_id: self.project_id,
            user_id: self.user_id,
            provider_account_id: self.provider_account_id,
            provider_credential_ref: self.provider_credential_ref,
            protocol_family: self.protocol_family,
            protocol_profile: self.protocol_profile,
            endpoint_kind: self.endpoint_kind,
            requested_model: self.requested_model,
            resolved_model: self.resolved_model,
            status: self.status,
            failure_class: self.failure_class,
            created_from: self.created_from,
            created_to: self.created_to,
            limit: self.limit,
        }
    }
}

async fn read_optional_artifact(object_key: &str) -> Option<Value> {
    let storage = gateway_object_storage().ok()?;
    storage.read_json(object_key).await.ok()
}
