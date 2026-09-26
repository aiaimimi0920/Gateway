use super::*;

#[test]
fn gemini_canvas_web_reverse_modular_supports_text_and_tts_ingress() {
    let payload = ProviderAccountPayload {
        adapter: "gemini_canvas_web_reverse_compatible".to_string(),
        ..make_candidate("gemini_canvas").payload
    };
    for endpoint_kind in [
        EndpointKind::ChatCompletions,
        EndpointKind::Completions,
        EndpointKind::Messages,
        EndpointKind::Responses,
        EndpointKind::AudioSpeech,
    ] {
        let families =
            request_compatible_wire_protocol_families(&payload, endpoint_kind, "gemini_canvas");
        assert_eq!(families, vec![GEMINI_GENERATE_CONTENT_FAMILY.to_string()]);
    }
}

#[test]
fn gemini_canvas_program_web_reverse_modular_supports_text_tts_and_generation_ingress() {
    let payload = ProviderAccountPayload {
        adapter: "gemini_canvas_program_web_reverse_compatible".to_string(),
        ..make_candidate("gemini_canvas").payload
    };

    for endpoint_kind in [
        EndpointKind::ChatCompletions,
        EndpointKind::Completions,
        EndpointKind::Messages,
        EndpointKind::Responses,
        EndpointKind::AudioSpeech,
    ] {
        let families =
            request_compatible_wire_protocol_families(&payload, endpoint_kind, "gemini_canvas");
        assert_eq!(families, vec![GEMINI_GENERATE_CONTENT_FAMILY.to_string()]);
    }

    assert_eq!(
        request_compatible_wire_protocol_families(
            &payload,
            EndpointKind::ImagesGenerations,
            "gemini_canvas"
        ),
        vec![GEMINI_CANVAS_IMAGES_FAMILY.to_string()]
    );
    assert_eq!(
        request_compatible_wire_protocol_families(
            &payload,
            EndpointKind::ImagesEdits,
            "gemini_canvas"
        ),
        Vec::<String>::new()
    );
    assert_eq!(
        request_compatible_wire_protocol_families(
            &payload,
            EndpointKind::MusicGenerations,
            "gemini_canvas"
        ),
        vec![GEMINI_CANVAS_MUSIC_FAMILY.to_string()]
    );
    assert_eq!(
        request_compatible_wire_protocol_families(
            &payload,
            EndpointKind::VideosGenerations,
            "gemini_canvas"
        ),
        vec![GEMINI_CANVAS_VIDEOS_FAMILY.to_string()]
    );
}

#[test]
fn gemini_api_modular_supports_cross_family_text_ingress() {
    let payload = ProviderAccountPayload {
        adapter: "gemini_api_modular_compatible".to_string(),
        ..make_candidate("gemini_generate_content").payload
    };
    for endpoint_kind in [
        EndpointKind::ChatCompletions,
        EndpointKind::Completions,
        EndpointKind::Messages,
        EndpointKind::Responses,
    ] {
        let families = request_compatible_wire_protocol_families(
            &payload,
            endpoint_kind,
            "gemini_generate_content",
        );
        assert_eq!(families, vec![GEMINI_GENERATE_CONTENT_FAMILY.to_string()]);
    }
}

#[test]
fn gemini_api_modular_supports_media_and_tts_endpoint_families() {
    let payload = ProviderAccountPayload {
        adapter: "gemini_api_modular_compatible".to_string(),
        ..make_candidate("gemini_generate_content").payload
    };
    assert_eq!(
        request_compatible_wire_protocol_families(
            &payload,
            EndpointKind::AudioSpeech,
            "gemini_generate_content",
        ),
        vec![GEMINI_GENERATE_CONTENT_FAMILY.to_string()]
    );
    assert_eq!(
        request_compatible_wire_protocol_families(
            &payload,
            EndpointKind::ImagesGenerations,
            "gemini_generate_content",
        ),
        vec![GEMINI_CANVAS_IMAGES_FAMILY.to_string()]
    );
    assert_eq!(
        request_compatible_wire_protocol_families(
            &payload,
            EndpointKind::MusicGenerations,
            "gemini_generate_content",
        ),
        vec![GEMINI_CANVAS_MUSIC_FAMILY.to_string()]
    );
    assert_eq!(
        request_compatible_wire_protocol_families(
            &payload,
            EndpointKind::VideosGenerations,
            "gemini_generate_content",
        ),
        vec![GEMINI_CANVAS_VIDEOS_FAMILY.to_string()]
    );
}

#[test]
fn aistudio_web_reverse_supports_cross_family_text_ingress() {
    let payload = ProviderAccountPayload {
        adapter: "aistudio_web_reverse_compatible".to_string(),
        ..make_candidate("gemini_generate_content").payload
    };
    for endpoint_kind in [
        EndpointKind::ChatCompletions,
        EndpointKind::Completions,
        EndpointKind::Messages,
        EndpointKind::Responses,
    ] {
        let families = request_compatible_wire_protocol_families(
            &payload,
            endpoint_kind,
            "gemini_generate_content",
        );
        assert_eq!(families, vec![GEMINI_GENERATE_CONTENT_FAMILY.to_string()]);
    }
}

#[test]
fn aistudio_web_reverse_supports_embeddings_ingress() {
    let payload = ProviderAccountPayload {
        adapter: "aistudio_web_reverse_compatible".to_string(),
        ..make_candidate("gemini_generate_content").payload
    };
    let families = request_compatible_wire_protocol_families(
        &payload,
        EndpointKind::Embeddings,
        "gemini_generate_content",
    );
    assert_eq!(families, vec![OPENAI_EMBEDDINGS_FAMILY.to_string()]);
}

#[test]
fn aistudio_web_reverse_supports_audio_speech_ingress() {
    let payload = ProviderAccountPayload {
        adapter: "aistudio_web_reverse_compatible".to_string(),
        ..make_candidate("gemini_generate_content").payload
    };
    let families = request_compatible_wire_protocol_families(
        &payload,
        EndpointKind::AudioSpeech,
        "gemini_generate_content",
    );
    assert_eq!(families, vec![OPENAI_AUDIO_SPEECH_FAMILY.to_string()]);
}

#[test]
fn aistudio_web_reverse_supports_image_generation_ingress() {
    let payload = ProviderAccountPayload {
        adapter: "aistudio_web_reverse_compatible".to_string(),
        ..make_candidate("gemini_generate_content").payload
    };
    let families = request_compatible_wire_protocol_families(
        &payload,
        EndpointKind::ImagesGenerations,
        "gemini_generate_content",
    );
    assert_eq!(families, vec![GEMINI_GENERATE_CONTENT_FAMILY.to_string()]);
}

#[test]
fn gemini_web_reverse_modular_supports_legacy_mixed_lane_tts_and_media_ingress() {
    let payload = ProviderAccountPayload {
        adapter: "gemini_web_reverse_modular_compatible".to_string(),
        ..make_candidate("gemini_web_chat").payload
    };

    assert_eq!(
        request_compatible_wire_protocol_families(
            &payload,
            EndpointKind::AudioSpeech,
            "gemini_web_chat",
        ),
        vec![GEMINI_GENERATE_CONTENT_FAMILY.to_string()]
    );
    assert_eq!(
        request_compatible_wire_protocol_families(
            &payload,
            EndpointKind::ImagesGenerations,
            "gemini_web_chat",
        ),
        vec![GEMINI_CANVAS_IMAGES_FAMILY.to_string()]
    );
    assert_eq!(
        request_compatible_wire_protocol_families(
            &payload,
            EndpointKind::ImagesEdits,
            "gemini_web_chat",
        ),
        vec![GEMINI_CANVAS_IMAGES_FAMILY.to_string()]
    );
    assert_eq!(
        request_compatible_wire_protocol_families(
            &payload,
            EndpointKind::MusicGenerations,
            "gemini_web_chat",
        ),
        vec![GEMINI_CANVAS_MUSIC_FAMILY.to_string()]
    );
    assert_eq!(
        request_compatible_wire_protocol_families(
            &payload,
            EndpointKind::VideosGenerations,
            "gemini_web_chat",
        ),
        vec![GEMINI_CANVAS_VIDEOS_FAMILY.to_string()]
    );
}
