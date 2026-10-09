//! Request-local protocol choice: no mutation of shared discovery state or network I/O.
use super::DiscoveredProtocol;
use crate::{
    protocol::canonical::CanonicalRelayRequest,
    routing::{
        candidate::{ProviderAccountPayload, RouteCandidate},
        protocol_resolution::{
            requested_wire_protocol_family, SAME_PROTOCOL_FAMILY_PRIORITY_BONUS,
        },
    },
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolCapability {
    pub protocol: DiscoveredProtocol,
    pub api_base: String,
    pub verified_models: Vec<String>,
    #[serde(default)]
    pub failed_models: Vec<String>,
}

impl ProtocolCapability {
    pub fn apply(&self, payload: &mut ProviderAccountPayload) {
        payload.base_url = self.api_base.clone();
        payload.chat_completions_path = None;
        payload.responses_path = None;
        payload.completions_path = None;
        payload.messages_path = None;
        match self.protocol {
            DiscoveredProtocol::DashscopeText | DiscoveredProtocol::DashscopeMultimodal => {
                let multimodal = self.protocol == DiscoveredProtocol::DashscopeMultimodal;
                payload.adapter = if multimodal {
                    "dashscope_multimodal_compatible"
                } else {
                    "dashscope_compatible"
                }
                .into();
                payload.chat_completions_path = Some(
                    if multimodal {
                        crate::protocol::dashscope::MULTIMODAL_PATH
                    } else {
                        crate::protocol::dashscope::TEXT_PATH
                    }
                    .into(),
                );
            }
            DiscoveredProtocol::ChatCompletions => {
                payload.adapter = "openai_compatible".into();
                payload.chat_completions_path = Some("/chat/completions".into());
            }
            DiscoveredProtocol::Responses => {
                payload.adapter = "openai_compatible".into();
                payload.responses_path = Some("/responses".into());
            }
            DiscoveredProtocol::Messages => {
                payload.adapter = "anthropic_compatible".into();
                payload.messages_path = Some("/messages".into());
            }
            _ => {
                payload.adapter = "discovery_only".into();
            }
        }
    }
}

pub fn select_protocol(
    candidate: &mut RouteCandidate,
    req: &CanonicalRelayRequest,
) -> Option<bool> {
    select_protocol_with_override(candidate, req, None)
}

pub fn select_protocol_with_override(
    candidate: &mut RouteCandidate,
    req: &CanonicalRelayRequest,
    forced: Option<&str>,
) -> Option<bool> {
    use crate::protocol::canonical::EndpointKind;
    if !matches!(
        req.endpoint_kind,
        EndpointKind::ChatCompletions
            | EndpointKind::Responses
            | EndpointKind::Messages
            | EndpointKind::Completions
    ) {
        return None;
    }
    let requested = requested_wire_protocol_family(req);
    let model = candidate.upstream_model.as_deref();
    // Failed sample evidence is not a universal unsupported claim, but must not be
    // overridden by account-level capability inferred from another model.
    let selected = candidate
        .payload
        .discovered_protocols
        .iter()
        .filter(|c| c.protocol.routing_ready())
        .filter(|c| forced.is_none_or(|target| target == c.protocol.family()))
        .filter(|c| model.is_none_or(|m| !c.failed_models.iter().any(|f| f == m)))
        .filter(|c| {
            candidate.supported_protocol_families.is_empty()
                || candidate.supported_protocol_families.iter().any(|family| {
                    crate::protocol::registry::canonicalize_wire_protocol_family_key(family)
                        == c.protocol.family()
                })
        })
        .filter(|c| requested.as_deref() == Some(c.protocol.family()) || conversion_supported(req))
        .min_by_key(|c| {
            let native = requested.as_deref() == Some(c.protocol.family());
            let verified = model.is_some_and(|m| c.verified_models.iter().any(|v| v == m));
            // Native wins unless this model has explicit negative evidence above.
            let confidence = match (native, verified) {
                (true, true) => 0,
                (true, false) => 1,
                (false, true) => 2,
                _ => 3,
            };
            let preference = match (req.endpoint_kind, c.protocol) {
                (
                    EndpointKind::Responses | EndpointKind::Messages,
                    DiscoveredProtocol::Responses,
                ) => 0,
                (_, DiscoveredProtocol::ChatCompletions) => 1,
                _ => 2,
            };
            (confidence, preference)
        })?
        .clone();
    selected.apply(&mut candidate.payload);
    candidate.adapter = candidate.payload.adapter.clone();
    candidate.protocol_family = selected.protocol.family().into();
    candidate.protocol_profile = match selected.protocol {
        DiscoveredProtocol::Messages => "anthropic_messages",
        DiscoveredProtocol::DashscopeText | DiscoveredProtocol::DashscopeMultimodal => {
            "dashscope_native"
        }
        _ => "openai_compatible_generic",
    }
    .into();
    candidate.resolved_execution_mode = candidate.payload.resolve_execution_mode(req.endpoint_kind);
    let same = requested.as_deref() == Some(selected.protocol.family());
    if same {
        candidate.priority = candidate
            .priority
            .saturating_add(SAME_PROTOCOL_FAMILY_PRIORITY_BONUS);
    }
    Some(same)
}

// These features carry provider-owned state or non-function tool semantics that
// the canonical bridge cannot reconstruct. Never silently drop them on fallback.
fn conversion_supported(req: &CanonicalRelayRequest) -> bool {
    if crate::protocol::dashscope::validate_bridge(req).is_err() {
        return false;
    }
    let body = &req.raw_body;
    if ["previous_response_id", "conversation"]
        .iter()
        .any(|key| body.get(key).is_some_and(|v| !v.is_null()))
        || body.get("background").and_then(serde_json::Value::as_bool) == Some(true)
    {
        return false;
    }
    !body
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|tools| {
            tools.iter().any(|tool| {
                tool.get("type")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|kind| kind != "function")
            })
        })
}
