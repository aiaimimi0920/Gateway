mod endpoints;
mod media;
mod normalize;
mod request;
mod response;
mod stream;
mod transport;

pub use super::shared::{GEMINI_API_MODULAR_ADAPTER, GEMINI_API_MODULAR_PROFILE};
pub use endpoints::supports_endpoint;
pub use media::{
    aspect_ratio_from_request, build_audio_binary_response, build_image_request_body,
    build_music_client_content, build_music_generation_accepted_response,
    build_music_generation_config, build_music_generation_response,
    build_openai_images_response_from_bytes, build_text_request_body, build_tts_request_body,
    build_video_generation_accepted_response, build_video_generation_response,
    extract_audio_from_generate_content_response,
    extract_inline_image_from_generate_content_response, prefers_url_response,
    requested_output_count, resolve_official_image_model, resolve_official_music_model,
    resolve_official_video_model, video_aspect_ratio_from_request,
};
pub use normalize::normalize_generate_content;
pub use request::pack_request;
pub use response::parse_response;
pub(crate) use response::{
    build_generate_content_success, map_gemini_finish_reason,
    map_gemini_finish_reason_to_canonical, parse_gemini_tool_call,
};
pub use stream::{accumulate_gemini_stream, translate_openai_sse_to_gemini_stream};
pub use transport::{default_path, default_query};
