//! Management route previews and sticky-affinity inspection/reset.

use super::super::internal_gateway::assert_management_access;
use super::required_pg_pool;
use crate::db;
use crate::error::GatewayError;
use crate::http::extractors::OptionalBearerToken;
use crate::protocol::canonical::EndpointKind;
use crate::state::AppState;
use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::Json;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidatePreviewQuery {
    pub access_key_id: String,
    pub model: String,
    pub endpoint_kind: String,
    pub estimated_tokens: Option<u64>,
    pub explicit_session_key: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AffinityQuery {
    pub access_key_id: String,
    pub model: String,
    pub explicit_session_key: Option<String>,
}

pub(super) fn parse_endpoint_kind(raw: &str) -> Result<EndpointKind, GatewayError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "chat_completions" => Ok(EndpointKind::ChatCompletions),
        "completions" => Ok(EndpointKind::Completions),
        "embeddings" => Ok(EndpointKind::Embeddings),
        "images_generations" => Ok(EndpointKind::ImagesGenerations),
        "images_edits" => Ok(EndpointKind::ImagesEdits),
        "music_generations" => Ok(EndpointKind::MusicGenerations),
        "videos_generations" => Ok(EndpointKind::VideosGenerations),
        "audio_transcriptions" => Ok(EndpointKind::AudioTranscriptions),
        "audio_speech" => Ok(EndpointKind::AudioSpeech),
        "messages" => Ok(EndpointKind::Messages),
        "responses" => Ok(EndpointKind::Responses),
        "search" => Ok(EndpointKind::Search),
        "fetch" => Ok(EndpointKind::Fetch),
        "research_create" => Ok(EndpointKind::ResearchCreate),
        "research_list" => Ok(EndpointKind::ResearchList),
        "research_get" => Ok(EndpointKind::ResearchGet),
        "credits_balance" => Ok(EndpointKind::CreditsBalance),
        other => Err(GatewayError::bad_request(format!(
            "不支持的 endpointKind: {other}"
        ))),
    }
}

pub async fn preview_candidates(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Query(query): Query<CandidatePreviewQuery>,
) -> Result<Json<Vec<db::GatewayAccessCandidatePreviewView>>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    Ok(Json(
        db::preview_access_candidates(
            required_pg_pool(state.as_ref())?,
            &state.redis_pool,
            &query.access_key_id,
            &query.model,
            parse_endpoint_kind(&query.endpoint_kind)?,
            query.estimated_tokens.unwrap_or(1),
            query.explicit_session_key.as_deref(),
        )
        .await?,
    ))
}

pub async fn preview_route_decision(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Query(query): Query<CandidatePreviewQuery>,
) -> Result<Json<db::GatewayAccessRouteDecisionPreviewView>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    Ok(Json(
        db::preview_route_decision(
            required_pg_pool(state.as_ref())?,
            &state.redis_pool,
            &query.access_key_id,
            &query.model,
            parse_endpoint_kind(&query.endpoint_kind)?,
            query.estimated_tokens.unwrap_or(1),
            query.explicit_session_key.as_deref(),
        )
        .await?,
    ))
}

pub async fn inspect_affinity(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Query(query): Query<AffinityQuery>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    Ok(Json(serde_json::json!({
        "affinity": db::inspect_access_sticky_affinity(
            &state.redis_pool,
            &query.access_key_id,
            &query.model,
            query.explicit_session_key.as_deref(),
        ).await?
    })))
}

pub async fn reset_affinity(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
    Json(query): Json<AffinityQuery>,
) -> Result<Json<serde_json::Value>, GatewayError> {
    assert_management_access(state.as_ref(), token.as_deref(), &headers)?;
    db::reset_access_sticky_affinity(
        &state.redis_pool,
        &query.access_key_id,
        &query.model,
        query.explicit_session_key.as_deref(),
    )
    .await?;
    Ok(Json(serde_json::json!({ "success": true })))
}
