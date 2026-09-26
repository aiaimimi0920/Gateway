mod bootstrap;
mod direct_http_tts;
mod execution;
mod headers;
mod legacy_execution;
mod legacy_mixed_lane;
mod legacy_text;
mod legacy_tts;
mod request_plan;
mod surface_bridge;

pub use bootstrap::bootstrap_app;
pub use direct_http_tts::{
    execute_direct_http_tts_export, execute_direct_http_tts_followups,
    gemini_canvas_tts_direct_http_audio_unavailable_error,
    gemini_canvas_tts_direct_http_audio_unavailable_from_bodies,
    resolve_direct_http_tts_audio_response,
};
pub use execution::{execute, execute_stream};
pub use headers::build_headers;
pub use legacy_execution::execute_legacy_media;
pub use legacy_mixed_lane::{
    classify_legacy_mixed_lane_endpoint, force_legacy_mixed_lane_payload,
    is_legacy_mixed_lane_adapter, legacy_mixed_lane_execution_route,
    legacy_mixed_lane_media_payload, legacy_mixed_lane_payload_for_endpoint,
    legacy_mixed_lane_text_payload, legacy_mixed_lane_tts_payload,
    prompt_for_legacy_mixed_lane_media_request, supports_legacy_mixed_lane_media_endpoint,
    supports_legacy_mixed_lane_text_endpoint, supports_legacy_mixed_lane_tts_endpoint,
    LegacyMixedLaneEndpointClass, LegacyMixedLaneExecutionKind, LegacyMixedLaneExecutionRoute,
};
pub use legacy_text::{execute_legacy_text, execute_legacy_text_stream};
pub use legacy_tts::execute_legacy_tts;
pub use request_plan::unsupported_request_plan_error;
pub use surface_bridge::bootstrap_from_payload_cache;

const PROVIDER: &str = "gemini_web_compatible";
