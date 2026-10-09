//! Inference endpoints, compatibility aliases and per-family body limits.

use crate::http::middleware::body_limit_layer;
use crate::http::routes::{
    audio, bedrock, cohere, completions, embeddings, gemini, gemini_live, images, messages, models,
    music, realtime, responses, search, videos,
};
use crate::state::AppState;
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

pub(super) fn mount(router: Router<Arc<AppState>>, state: &AppState) -> Router<Arc<AppState>> {
    let router = router.merge(
        Router::new()
            .route(
                "/api/v1/services/aigc/text-generation/generation",
                post(crate::http::routes::dashscope::text),
            )
            .route(
                "/api/v1/services/aigc/multimodal-generation/generation",
                post(crate::http::routes::dashscope::multimodal),
            )
            .layer(body_limit_layer(
                state.config.max_body_chat_completions_bytes,
            )),
    );
    let chat_routes = Router::new()
        .route(
            "/v1/chat/completions",
            post(completions::handle_chat_completions),
        )
        .layer(body_limit_layer(
            state.config.max_body_chat_completions_bytes,
        ));

    let completions_routes = Router::new()
        .route(
            "/v1/completions",
            post(completions::handle_legacy_completions),
        )
        .layer(body_limit_layer(state.config.max_body_completions_bytes));

    let messages_routes = Router::new()
        .route("/v1/messages", post(messages::handle_messages))
        .layer(body_limit_layer(state.config.max_body_messages_bytes));

    let responses_routes = Router::new()
        .route("/v1/responses", post(responses::handle_responses))
        .layer(body_limit_layer(state.config.max_body_responses_bytes));

    let gemini_generate_content_routes = Router::new()
        .route("/v1/models/*action", post(gemini::handle_v1_model_action))
        .route(
            "/v1beta/models/*action",
            post(gemini::handle_v1beta_model_action),
        )
        .layer(body_limit_layer(
            state.config.max_body_chat_completions_bytes,
        ));

    let bedrock_converse_routes = Router::new()
        .route("/model/:model/converse", post(bedrock::handle_converse))
        .route(
            "/model/:model/converse-stream",
            post(bedrock::handle_converse_stream),
        )
        .layer(body_limit_layer(
            state.config.max_body_chat_completions_bytes,
        ));

    let cohere_chat_routes = Router::new()
        .route("/v2/chat", post(cohere::handle_chat_v2))
        .layer(body_limit_layer(
            state.config.max_body_chat_completions_bytes,
        ));

    let image_routes = Router::new()
        .route(
            "/v1/images/generations",
            post(images::handle_image_generations),
        )
        .layer(body_limit_layer(
            state.config.max_body_images_generations_bytes,
        ));

    let embeddings_routes = Router::new()
        .route("/v1/embeddings", post(embeddings::handle_embeddings))
        .layer(body_limit_layer(state.config.max_body_embeddings_bytes));

    let audio_transcription_routes = Router::new()
        .route(
            "/v1/audio/transcriptions",
            post(audio::handle_audio_transcriptions),
        )
        .layer(body_limit_layer(
            state.config.max_body_audio_transcriptions_bytes,
        ));

    let audio_speech_routes = Router::new()
        .route("/v1/audio/speech", post(audio::handle_audio_speech))
        .layer(body_limit_layer(state.config.max_body_audio_speech_bytes));

    let image_edit_routes = Router::new()
        .route("/v1/images/edits", post(images::handle_image_edits))
        .layer(body_limit_layer(state.config.max_body_images_edits_bytes));

    let search_routes = Router::new()
        .route("/v1/search", post(search::handle_search))
        .layer(body_limit_layer(state.config.max_body_search_bytes));

    let fetch_routes = Router::new()
        .route("/v1/fetch", post(search::handle_fetch))
        .layer(body_limit_layer(state.config.max_body_fetch_bytes));

    let research_routes = Router::new()
        .route(
            "/v1/research",
            get(search::handle_research_list).post(search::handle_research_create),
        )
        .layer(body_limit_layer(state.config.max_body_research_bytes));

    let music_routes = Router::new()
        .route(
            "/v1/music/generations",
            post(music::handle_music_generations),
        )
        .layer(body_limit_layer(state.config.max_body_music_bytes));

    let video_routes = Router::new()
        .route(
            "/v1/videos/generations",
            post(videos::handle_video_generations),
        )
        .layer(body_limit_layer(state.config.max_body_videos_bytes));

    let new_api_chat_routes = Router::new()
        .route(
            "/v1/new-api/chat/completions",
            post(completions::handle_chat_completions),
        )
        .layer(body_limit_layer(
            state.config.max_body_chat_completions_bytes,
        ));

    let new_api_completions_routes = Router::new()
        .route(
            "/v1/new-api/completions",
            post(completions::handle_legacy_completions),
        )
        .layer(body_limit_layer(state.config.max_body_completions_bytes));

    let new_api_messages_routes = Router::new()
        .route("/v1/new-api/messages", post(messages::handle_messages))
        .layer(body_limit_layer(state.config.max_body_messages_bytes));

    let new_api_responses_routes = Router::new()
        .route("/v1/new-api/responses", post(responses::handle_responses))
        .layer(body_limit_layer(state.config.max_body_responses_bytes));

    let new_api_image_routes = Router::new()
        .route(
            "/v1/new-api/images/generations",
            post(images::handle_image_generations),
        )
        .layer(body_limit_layer(
            state.config.max_body_images_generations_bytes,
        ));

    let new_api_embeddings_routes = Router::new()
        .route(
            "/v1/new-api/embeddings",
            post(embeddings::handle_embeddings),
        )
        .layer(body_limit_layer(state.config.max_body_embeddings_bytes));

    let new_api_audio_transcription_routes = Router::new()
        .route(
            "/v1/new-api/audio/transcriptions",
            post(audio::handle_audio_transcriptions),
        )
        .layer(body_limit_layer(
            state.config.max_body_audio_transcriptions_bytes,
        ));

    let new_api_audio_speech_routes = Router::new()
        .route("/v1/new-api/audio/speech", post(audio::handle_audio_speech))
        .layer(body_limit_layer(state.config.max_body_audio_speech_bytes));

    let new_api_image_edit_routes = Router::new()
        .route("/v1/new-api/images/edits", post(images::handle_image_edits))
        .layer(body_limit_layer(state.config.max_body_images_edits_bytes));

    let new_api_search_routes = Router::new()
        .route("/v1/new-api/search", post(search::handle_search))
        .layer(body_limit_layer(state.config.max_body_search_bytes));

    let new_api_fetch_routes = Router::new()
        .route("/v1/new-api/fetch", post(search::handle_fetch))
        .layer(body_limit_layer(state.config.max_body_fetch_bytes));

    let new_api_research_routes = Router::new()
        .route(
            "/v1/new-api/research",
            get(search::handle_research_list).post(search::handle_research_create),
        )
        .layer(body_limit_layer(state.config.max_body_research_bytes));

    let new_api_music_routes = Router::new()
        .route(
            "/v1/new-api/music/generations",
            post(music::handle_music_generations),
        )
        .layer(body_limit_layer(state.config.max_body_music_bytes));

    let new_api_video_routes = Router::new()
        .route(
            "/v1/new-api/videos/generations",
            post(videos::handle_video_generations),
        )
        .layer(body_limit_layer(state.config.max_body_videos_bytes));

    router
        // -- AI endpoints ------------------------------------------------
        .merge(chat_routes)
        .merge(completions_routes)
        .merge(messages_routes)
        .merge(responses_routes)
        .merge(gemini_generate_content_routes)
        .merge(bedrock_converse_routes)
        .merge(cohere_chat_routes)
        .merge(embeddings_routes)
        .merge(audio_transcription_routes)
        .merge(audio_speech_routes)
        .merge(image_routes)
        .merge(image_edit_routes)
        .merge(search_routes)
        .merge(fetch_routes)
        .merge(research_routes)
        .merge(music_routes)
        .merge(video_routes)
        .merge(new_api_chat_routes)
        .merge(new_api_completions_routes)
        .merge(new_api_messages_routes)
        .merge(new_api_responses_routes)
        .merge(new_api_embeddings_routes)
        .merge(new_api_audio_transcription_routes)
        .merge(new_api_audio_speech_routes)
        .merge(new_api_image_routes)
        .merge(new_api_image_edit_routes)
        .merge(new_api_search_routes)
        .merge(new_api_fetch_routes)
        .merge(new_api_research_routes)
        .merge(new_api_music_routes)
        .merge(new_api_video_routes)
        .route("/v1/research/:id", get(search::handle_research_get))
        .route("/v1/new-api/research/:id", get(search::handle_research_get))
        .route("/v1/credits/balance", get(search::handle_credits_balance))
        .route(
            "/v1/new-api/credits/balance",
            get(search::handle_credits_balance),
        )
        .route(
            "/v1/internal/providers/search/credits/balance",
            get(search::handle_internal_search_provider_balance),
        )
        // Legacy compatibility alias for older operator clients.
        .route(
            "/v1/internal/providers/linkup/credits/balance",
            get(search::handle_internal_search_provider_balance),
        )
        .route("/v1/models", get(models::handle_models))
        .route("/v1/new-api/models", get(models::handle_models))
        .route("/v1/realtime", get(realtime::handle_realtime))
        .route(
            "/ws/google.ai.generativelanguage.v1alpha.GenerativeService/BidiGenerateContent",
            get(gemini_live::handle_bidi_generate_content),
        )
        .route(
            "/ws/google.ai.generativelanguage.v1beta.GenerativeService/BidiGenerateContent",
            get(gemini_live::handle_bidi_generate_content),
        )
}
