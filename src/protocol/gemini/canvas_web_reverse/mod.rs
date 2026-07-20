mod models;
mod prompt;
mod relay_config;
mod request_builders;
mod request_context;
mod request_spec;
mod response_builders;
mod response_parsers;
mod stream_parsers;

pub use super::shared::{
    GEMINI_CANVAS_WEB_REVERSE_MODULAR_ADAPTER, GEMINI_CANVAS_WEB_REVERSE_MODULAR_PROFILE,
};
pub use models::{
    resolve_direct_http_image_model, resolve_image_model, resolve_music_model,
    resolve_official_image_model, resolve_official_music_model, resolve_official_video_model,
    resolve_text_model, resolve_video_model,
};
pub(crate) use prompt::latest_nonempty_user_text;
pub use prompt::{prompt_for_text_request, prompt_from_request};
pub use relay_config::{
    relay_config_from_payload, GeminiCanvasBrowserRelayConfig,
    GEMINI_CANVAS_BROWSER_RELAY_DEFAULT_WS_PATH, GEMINI_CANVAS_BROWSER_RELAY_PROVIDER_KEY,
};
pub use request_builders::{
    build_direct_http_image_request_body, build_image_request_body, build_imagen_predict_request,
    build_music_client_content, build_text_request_body, build_tts_request_body,
};
pub use request_context::{
    aspect_ratio_from_request, build_text_fetch_url, direct_http_image_api_base_url,
    locale_from_payload, prefers_url_response, requested_output_count, runtime_from_payload,
};
pub use request_spec::{
    build_browser_executor_invocation_input, build_browser_relay_request_spec,
    GeminiCanvasBrowserRelayRequestSpec,
};
pub use response_builders::{
    build_music_generation_response, build_openai_images_response_from_bytes,
    build_openai_images_response_from_urls, build_video_generation_response,
};
pub use response_parsers::{
    build_audio_binary_response, extract_audio_from_generate_content_response,
    extract_images_from_imagen_predict_response,
    extract_inline_image_from_generate_content_response, requested_tts_response_format,
};
pub use stream_parsers::{
    extract_audio_from_tts_export_response, extract_conversation_list_entries,
    extract_stream_generate_locator, extract_stream_generate_response_id,
    extract_video_generation_job_id, response_indicates_video_generation_pending,
    response_indicates_video_generation_quota_reached,
};

#[cfg(test)]
mod tests;
