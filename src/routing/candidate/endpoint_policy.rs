//! Endpoint capabilities, protocol bridging and execution-mode policy.

use super::{
    is_search_api_adapter_name, ProviderAccountPayload, ProviderExecutionMode,
    AISTUDIO_WEB_REVERSE_COMPATIBLE_ADAPTER, GEMINI_CANVAS_WEB_REVERSE_MODULAR_COMPATIBLE_ADAPTER,
    SEARCH_API_COMPATIBLE_ADAPTER,
};
use crate::protocol::canonical::EndpointKind;

impl ProviderAccountPayload {
    /// Passthrough only for a discovered, selected native wire with untouched tools.
    pub fn is_discovered_native(
        &self,
        req: &crate::protocol::canonical::CanonicalRelayRequest,
    ) -> bool {
        if matches!(
            (req.protocol_family, self.adapter.as_str()),
            (
                crate::protocol::canonical::ProtocolFamily::DashScope,
                "dashscope_compatible"
            ) | (
                crate::protocol::canonical::ProtocolFamily::DashScopeMultimodal,
                "dashscope_multimodal_compatible"
            )
        ) {
            return true;
        }
        if self.discovered_protocols.is_empty() {
            return false;
        }
        let requested = crate::routing::protocol_resolution::requested_wire_protocol_family(req);
        self.discovered_protocols.iter().any(|c| {
            requested.as_deref() == Some(c.protocol.family())
                && self.base_url == c.api_base
                && match c.protocol {
                    crate::provider_discovery::DiscoveredProtocol::ChatCompletions => {
                        self.adapter == "openai_compatible"
                            && self.chat_completions_path.is_some()
                            && self.responses_path.is_none()
                    }
                    crate::provider_discovery::DiscoveredProtocol::Responses => {
                        self.adapter == "openai_compatible"
                            && self.responses_path.is_some()
                            && self.chat_completions_path.is_none()
                    }
                    crate::provider_discovery::DiscoveredProtocol::Messages => {
                        self.adapter == "anthropic_compatible"
                    }
                    _ => false,
                }
        })
    }
    pub fn canonical_adapter(&self) -> &str {
        if is_search_api_adapter_name(&self.adapter) {
            SEARCH_API_COMPATIBLE_ADAPTER
        } else {
            self.adapter.as_str()
        }
    }

    pub fn bridges_openai_text_endpoint_to_responses(&self, endpoint_kind: EndpointKind) -> bool {
        if self.canonical_adapter() != "openai_compatible" || self.responses_path.is_none() {
            return false;
        }

        match endpoint_kind {
            EndpointKind::ChatCompletions => self.chat_completions_path.is_none(),
            EndpointKind::Messages => true,
            EndpointKind::Completions => {
                self.completions_path.is_none() && self.chat_completions_path.is_none()
            }
            _ => false,
        }
    }

    pub fn bridges_openai_responses_to_chat_completions(
        &self,
        endpoint_kind: EndpointKind,
    ) -> bool {
        if self.canonical_adapter() != "openai_compatible" {
            return false;
        }

        matches!(endpoint_kind, EndpointKind::Responses)
            && self.responses_path.is_none()
            && self.chat_completions_path.is_some()
    }

    pub fn prefers_forced_streaming_responses(&self, endpoint_kind: EndpointKind) -> bool {
        if !self.discovered_protocols.is_empty() && endpoint_kind == EndpointKind::Responses {
            return false;
        }
        if self.canonical_adapter() != "openai_compatible" {
            return false;
        }

        if self.bridges_openai_text_endpoint_to_responses(endpoint_kind) {
            return true;
        }

        matches!(endpoint_kind, EndpointKind::Responses)
            && self.responses_path.is_some()
            && self.chat_completions_path.is_none()
    }

    pub fn supports_search_endpoint(&self, endpoint_kind: EndpointKind) -> bool {
        match endpoint_kind {
            EndpointKind::Search => self.search_path.is_some(),
            EndpointKind::Fetch => self.fetch_path.is_some(),
            EndpointKind::ResearchCreate
            | EndpointKind::ResearchList
            | EndpointKind::ResearchGet => self.research_path.is_some(),
            EndpointKind::CreditsBalance => self.balance_path.is_some(),
            EndpointKind::ChatCompletions
            | EndpointKind::Completions
            | EndpointKind::Embeddings
            | EndpointKind::ImagesGenerations
            | EndpointKind::ImagesEdits
            | EndpointKind::MusicGenerations
            | EndpointKind::VideosGenerations
            | EndpointKind::AudioTranscriptions
            | EndpointKind::AudioSpeech
            | EndpointKind::Messages
            | EndpointKind::Responses => true,
        }
    }

    pub fn resolve_execution_mode(&self, endpoint_kind: EndpointKind) -> ProviderExecutionMode {
        let endpoint_key = endpoint_kind_key(endpoint_kind);
        if let Some(mode) = self
            .endpoint_execution_modes
            .as_ref()
            .and_then(|overrides| overrides.get(endpoint_key))
            .copied()
        {
            return mode;
        }
        if self.adapter == "producer_compatible"
            && matches!(endpoint_kind, EndpointKind::VideosGenerations)
        {
            return ProviderExecutionMode::BrowserBacked;
        }
        self.execution_mode
            .unwrap_or_else(|| default_execution_mode_for_adapter(&self.adapter))
    }
}

fn default_execution_mode_for_adapter(adapter: &str) -> ProviderExecutionMode {
    match adapter {
        "lumalabs_compatible"
        | "udio_compatible"
        | GEMINI_CANVAS_WEB_REVERSE_MODULAR_COMPATIBLE_ADAPTER
        | AISTUDIO_WEB_REVERSE_COMPATIBLE_ADAPTER => ProviderExecutionMode::BrowserBacked,
        _ => ProviderExecutionMode::DirectHttp,
    }
}

fn endpoint_kind_key(endpoint_kind: EndpointKind) -> &'static str {
    match endpoint_kind {
        EndpointKind::ChatCompletions => "chat_completions",
        EndpointKind::Completions => "completions",
        EndpointKind::Embeddings => "embeddings",
        EndpointKind::ImagesGenerations => "images_generations",
        EndpointKind::ImagesEdits => "images_edits",
        EndpointKind::MusicGenerations => "music_generations",
        EndpointKind::VideosGenerations => "videos_generations",
        EndpointKind::AudioTranscriptions => "audio_transcriptions",
        EndpointKind::AudioSpeech => "audio_speech",
        EndpointKind::Messages => "messages",
        EndpointKind::Responses => "responses",
        EndpointKind::Search => "search",
        EndpointKind::Fetch => "fetch",
        EndpointKind::ResearchCreate => "research_create",
        EndpointKind::ResearchList => "research_list",
        EndpointKind::ResearchGet => "research_get",
        EndpointKind::CreditsBalance => "credits_balance",
    }
}
