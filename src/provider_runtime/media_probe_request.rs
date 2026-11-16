//! One explicitly charged manual media operation, never a saved/automatic text plan.
use super::ConsoleProbeRequest;
use crate::{error::GatewayError, protocol::canonical::CanonicalRelayRequest};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Clone, Copy)]
pub(super) enum MediaProbeProduct {
    LumaAgents,
    LumaWeb,
    Chataibot,
    Suno,
    Udio,
}
impl MediaProbeProduct {
    pub(super) fn for_payload(
        payload: &crate::routing::candidate::ProviderAccountPayload,
    ) -> Option<Self> {
        match payload.canonical_adapter() {
            crate::protocol::luma_agents::ADAPTER => Some(Self::LumaAgents),
            "lumalabs_compatible" => Some(Self::LumaWeb),
            "chataibot_compatible" => Some(Self::Chataibot),
            "suno_compatible" => Some(Self::Suno),
            "udio_compatible" => Some(Self::Udio),
            _ => None,
        }
    }
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::LumaAgents => "Luma Agents",
            Self::LumaWeb => "Luma Web",
            Self::Chataibot => "ChatAIBot",
            Self::Suno => "Suno",
            Self::Udio => "Udio",
        }
    }
    pub(super) fn family(self) -> &'static str {
        match self {
            Self::LumaAgents => crate::protocol::luma_agents::FAMILY,
            Self::LumaWeb => "lumalabs_web",
            Self::Chataibot => "chataibot_web",
            Self::Suno => "suno_web",
            Self::Udio => "udio_web",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MediaProbeOperation {
    Image,
    ImageEdit,
    Video,
    Music,
}
impl MediaProbeOperation {
    pub(super) fn endpoint(self) -> &'static str {
        match self {
            Self::Image => "images_generations",
            Self::ImageEdit => "images_edits",
            Self::Video => "videos_generations",
            Self::Music => "music_generations",
        }
    }
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MediaProbeRequest {
    pub operation: MediaProbeOperation,
    pub confirm_charge: bool,
    #[serde(default)]
    pub parameters: Map<String, Value>,
}
impl MediaProbeRequest {
    pub(super) fn valid(&self, request: &ConsoleProbeRequest) -> bool {
        self.confirm_charge
            && request.model.is_some()
            && request
                .credential_ids
                .as_ref()
                .is_some_and(|ids| ids.len() == 1)
            && request.scope.is_none()
            && request.plan_id.is_none()
            && request.test_plan.is_none()
            && !self.parameters.keys().any(|key| {
                ["model", "prompt", "type", "n", "stream", "response_format"]
                    .contains(&key.as_str())
            })
    }
    pub(super) fn canonical(
        &self,
        request: &ConsoleProbeRequest,
        resolved_model: &str,
        payload: &crate::routing::candidate::ProviderAccountPayload,
        product: MediaProbeProduct,
    ) -> Result<CanonicalRelayRequest, GatewayError> {
        if !self.valid(request) {
            return Err(crate::protocol::luma_agents::invalid_request());
        }
        if let MediaProbeProduct::Suno = product {
            return super::suno_media_probe::canonical(self, request, resolved_model, payload);
        }
        if let MediaProbeProduct::Udio = product {
            return super::udio_media_probe::canonical(self, request, resolved_model, payload);
        }
        if let MediaProbeProduct::LumaWeb = product {
            return super::lumalabs_media_probe::canonical(self, request, resolved_model, payload);
        }
        let mut body = self.parameters.clone();
        body.insert(
            "model".into(),
            Value::String(request.model.clone().unwrap()),
        );
        body.insert("prompt".into(), Value::String(request.prompt.clone()));
        // The ordinary sender still downloads and validates every asset, but this probe does not
        // return or base64-copy media into its text result/audit.
        let body = Value::Object(body);
        if let MediaProbeProduct::Chataibot = product {
            if !matches!(
                self.operation,
                MediaProbeOperation::Image | MediaProbeOperation::ImageEdit
            ) {
                return Err(crate::upstream::chataibot::unsupported_request_plan_error());
            }
            let mut canonical =
                crate::protocol::gemini_business::normalize_image_generations(body)?;
            if matches!(self.operation, MediaProbeOperation::ImageEdit) {
                canonical.endpoint_kind = crate::protocol::canonical::EndpointKind::ImagesEdits;
            }
            #[cfg(feature = "line-chataibot-web-reverse")]
            {
                crate::upstream::chataibot::validate_media_probe(
                    payload,
                    &canonical,
                    resolved_model,
                )?;
                return Ok(canonical);
            }
            #[cfg(not(feature = "line-chataibot-web-reverse"))]
            {
                let _ = (payload, canonical);
                return Err(crate::upstream::chataibot::unsupported_request_plan_error());
            }
        }
        let canonical = match self.operation {
            MediaProbeOperation::Image => {
                crate::protocol::gemini_business::normalize_image_generations(body)
            }
            MediaProbeOperation::ImageEdit => {
                crate::protocol::luma_agents::normalize_source_edit(body)
            }
            MediaProbeOperation::Video => {
                crate::protocol::gemini_canvas::normalize_video_generations(body)
            }
            MediaProbeOperation::Music => {
                return Err(crate::protocol::luma_agents::invalid_request())
            }
        }?;
        crate::protocol::luma_agents::GenerationRequest::pack(&canonical, resolved_model)?;
        Ok(canonical)
    }
}
