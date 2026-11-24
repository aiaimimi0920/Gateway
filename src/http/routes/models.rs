// ---------------------------------------------------------------------------
// http/routes/models.rs — GET /v1/models
//
// Returns the list of models available through the gateway, compatible with
// the OpenAI /v1/models response format.
// ---------------------------------------------------------------------------

use std::sync::Arc;

use axum::{extract::State, http::HeaderMap, response::IntoResponse, Json};
use serde::Serialize;

use crate::http::extractors::OptionalBearerToken;
use crate::http::request_headers::build_public_auth_request;
use crate::provider_runtime;
use crate::routing::config::ModelInfo;
use crate::state::AppState;

/// GET /v1/models — list available models.
///
/// Returns an OpenAI-compatible model list derived from the route
/// configuration (providers, exact routes, and aliases).
pub async fn handle_models(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    headers: HeaderMap,
) -> impl IntoResponse {
    let session = crate::auth::authenticate_request(
        state.as_ref(),
        &build_public_auth_request(
            &state.config,
            token.as_deref(),
            &headers,
            "/v1/models",
            "GET",
        ),
        headers
            .get("x-credential-ref")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string),
    )
    .await?;

    let local_id = crate::local_runtime::access_keys::local_key_id(&state, Some(&session));
    let route = state.route_config.snapshot();
    let admit = |p: &str, c: &str, m: &str| {
        crate::routing::admission::subject_allowed(&state, &route, p, Some(c), m)
    };
    let mut models = if let (Some(local), Some(id)) = (&state.local_runtime, local_id) {
        local
            .local_access_models_admitted(id, &state.route_config, &admit)
            .await?
    } else if let Some(pg_pool) = &state.pg_pool {
        provider_runtime::sweep_cooling_provider_accounts_best_effort(&state, 10).await;
        if let Some(access_key_id) = session.access_key_id.as_deref() {
            crate::db::list_models_for_access_key_admitted(
                pg_pool,
                &state.redis_pool,
                access_key_id,
                &|p, c, m| crate::routing::admission::subject_allowed(&state, &route, p, c, m),
            )
            .await?
        } else {
            crate::db::list_models_for_project_admitted(pg_pool, &session.project_id, &|p, c, m| {
                crate::routing::admission::subject_allowed(&state, &route, p, c, m)
            })
            .await?
        }
    } else {
        let mut models = route.list_models();
        models.retain(|m| route.has_admitted_candidate(&m.id, None, None, &admit));
        models
    };
    if local_id.is_none() {
        let snapshot = state.route_config.snapshot();
        if let Some(groups) =
            crate::access_key_groups::session_constraint(&state, Some(&session), &snapshot).await?
        {
            // Group restrictions narrow server entitlements; they never grant a missing bundle right.
            models.retain(|model| {
                snapshot.has_admitted_candidate(&model.id, Some(&groups), None, &admit)
            });
        }
    }
    Ok::<_, crate::error::GatewayError>(Json(PublicModelList {
        object: "list",
        data: models.into_iter().map(PublicModel::from).collect(),
    }))
}

#[derive(Serialize)]
struct PublicModelList {
    object: &'static str,
    data: Vec<PublicModel>,
}

#[derive(Serialize)]
struct PublicModel {
    #[serde(flatten)]
    info: ModelInfo,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    reasoning: Option<ReasoningMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    input_modalities: Option<&'static [&'static str]>,
}

impl From<ModelInfo> for PublicModel {
    fn from(info: ModelInfo) -> Self {
        // Annotate admitted public IDs only; do not infer from aliases or provider names.
        let reasoning = ReasoningMetadata::for_model(&info.id);
        let input_modalities: Option<&'static [&'static str]> = match info.id.as_str() {
            "gpt-6.2-sol" | "gpt-6.2-astra" => Some(&["text", "image"]),
            _ => None,
        };
        Self {
            info,
            reasoning,
            input_modalities,
        }
    }
}

#[derive(Serialize)]
#[serde(untagged)]
enum ReasoningMetadata {
    OpenAi {
        // Client selection hint, not a default injected into upstream requests.
        default_reasoning_level: &'static str,
        supported_reasoning_levels: &'static [ReasoningLevel],
    },
    Native {
        native_reasoning: NativeReasoning,
    },
}

impl ReasoningMetadata {
    // Model-level declarations, not route capability checks. See INTEGRATION_CONTRACT.md.
    fn for_model(id: &str) -> Option<Self> {
        let (default_reasoning_level, supported_reasoning_levels) = match id {
            "gpt-6.2-sol" | "gpt-6.2-astra" | "gpt-6.1-sol" => ("medium", GPT_6_REASONING_LEVELS),
            "gpt-5.5" => ("medium", GPT_5_REASONING_LEVELS),
            "gpt-5.4" | "gpt-5.4-mini" => ("none", GPT_5_REASONING_LEVELS),
            "claude-sonnet-4-6" => return Some(Self::anthropic(&["low", "medium", "high", "max"])),
            "claude-opus-4-7" | "claude-opus-4-8" => {
                return Some(Self::anthropic(&["low", "medium", "high", "xhigh", "max"]))
            }
            "gemini-3.1-pro-preview" => return Some(Self::gemini(&["LOW", "MEDIUM", "HIGH"])),
            "gemini-3-flash-preview" => {
                return Some(Self::gemini(&["MINIMAL", "LOW", "MEDIUM", "HIGH"]))
            }
            _ => return None,
        };
        Some(Self::OpenAi {
            default_reasoning_level,
            supported_reasoning_levels,
        })
    }

    fn anthropic(supported_levels: &'static [&'static str]) -> Self {
        Self::Native {
            native_reasoning: NativeReasoning {
                protocol: "anthropic_messages",
                parameter: "output_config.effort",
                supported_levels,
            },
        }
    }

    fn gemini(supported_levels: &'static [&'static str]) -> Self {
        Self::Native {
            native_reasoning: NativeReasoning {
                protocol: "gemini_generate_content",
                parameter: "generationConfig.thinkingConfig.thinkingLevel",
                supported_levels,
            },
        }
    }
}

#[derive(Serialize)]
struct NativeReasoning {
    protocol: &'static str,
    parameter: &'static str,
    supported_levels: &'static [&'static str],
}

#[derive(Serialize)]
struct ReasoningLevel {
    effort: &'static str,
    description: &'static str,
}

const LOW: ReasoningLevel = ReasoningLevel {
    effort: "low",
    description: "Faster responses with less reasoning",
};
const MEDIUM: ReasoningLevel = ReasoningLevel {
    effort: "medium",
    description: "Balance response speed and reasoning depth",
};
const HIGH: ReasoningLevel = ReasoningLevel {
    effort: "high",
    description: "Deeper reasoning for complex tasks",
};
const XHIGH: ReasoningLevel = ReasoningLevel {
    effort: "xhigh",
    description: "More reasoning for especially difficult tasks",
};

const GPT_6_REASONING_LEVELS: &[ReasoningLevel] = &[
    LOW,
    MEDIUM,
    HIGH,
    XHIGH,
    ReasoningLevel {
        effort: "max",
        description: "Highest reasoning effort for the hardest tasks",
    },
];

const GPT_5_REASONING_LEVELS: &[ReasoningLevel] = &[
    ReasoningLevel {
        effort: "none",
        description: "Respond without additional reasoning",
    },
    LOW,
    MEDIUM,
    HIGH,
    XHIGH,
];
