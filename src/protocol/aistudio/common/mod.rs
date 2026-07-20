mod config;
mod embeddings;
mod fixtures;
mod request_builders;

pub use config::{config_from_payload, AIStudioWebConfig, AISTUDIO_DEFAULT_APP_URL};
pub use embeddings::{
    bridge_embeddings_response, build_embeddings_request, build_fixture_embeddings_response,
    AIStudioEmbeddingsRequest,
};
pub use fixtures::{
    build_fixture_audio_binary_response, build_fixture_canonical_response, fixture_audio,
    fixture_image, AIStudioFixtureAudio, AIStudioFixtureImage,
};
pub use request_builders::{
    build_generate_content_url, build_image_request_body, build_text_request_body,
    build_tts_request_body,
};
