use super::*;

#[test]
fn harvest_text_batchexecute_header_id_reads_template_suffix() {
    let storage_state = json!({
        "textStreamGenerateTemplate": {
            "headers": {
                "x-goog-ext-525001261-jspb": "[1,null,null,null,\"fbb127bbb056c959\",null,null,0,[4],null,null,1,null,null,1,null,\"HEADER-ID\"]"
            }
        }
    });

    assert_eq!(
        harvest_text_batchexecute_header_id(&storage_state).as_deref(),
        Some("HEADER-ID")
    );
}

#[test]
fn harvest_batchexecute_header_id_for_mode_prefers_image_template_for_image_mode() {
    let storage_state = json!({
        "textStreamGenerateTemplate": {
            "headers": {
                "x-goog-ext-525001261-jspb": "[1,null,null,null,\"fbb127bbb056c959\",null,null,0,[4],null,null,1,null,null,1,null,\"TEXT-ID\"]"
            }
        },
        "imageStreamGenerateTemplate": {
            "headers": {
                "x-goog-ext-525001261-jspb": "[1,null,null,null,\"56fdd199312815e2\",null,null,0,[4],null,null,2,null,null,1,null,\"IMAGE-ID\"]"
            }
        }
    });

    assert_eq!(
        harvest_batchexecute_header_id_for_mode(
            &storage_state,
            GEMINI_CANVAS_TEXT_STREAM_GENERATE_TEXT_MODE_INDEX
        )
        .as_deref(),
        Some("TEXT-ID")
    );
    assert_eq!(
        harvest_batchexecute_header_id_for_mode(
            &storage_state,
            GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX
        )
        .as_deref(),
        Some("IMAGE-ID")
    );
}

#[test]
fn build_stream_generate_model_header_from_storage_state_preserves_suffix_and_switches_mode() {
    let storage_state = json!({
        "textStreamGenerateTemplate": {
            "headers": {
                "x-goog-ext-525001261-jspb": "[1,null,null,null,\"fbb127bbb056c959\",null,null,0,[4],null,null,1,null,null,1,null,\"HEADER-ID\"]"
            }
        }
    });

    let media_header = build_stream_generate_model_header_from_storage_state(
        &storage_state,
        GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX,
        false,
    )
    .unwrap();
    let media = serde_json::from_str::<Vec<Value>>(&media_header).unwrap();
    assert_eq!(
        media[4].as_str(),
        Some(GEMINI_CANVAS_TEXT_SELECTED_MODEL_HEADER_ID)
    );
    assert_eq!(media[11].as_i64(), Some(2));
    assert_eq!(media[16].as_str(), Some("HEADER-ID"));

    let text_header = build_stream_generate_model_header_from_storage_state(
        &storage_state,
        GEMINI_CANVAS_TEXT_STREAM_GENERATE_TEXT_MODE_INDEX,
        false,
    )
    .unwrap();
    let text = serde_json::from_str::<Vec<Value>>(&text_header).unwrap();
    assert_eq!(
        text[4].as_str(),
        Some(GEMINI_CANVAS_TEXT_LAST_SELECTED_MODE_ID)
    );
    assert_eq!(text[11].as_i64(), Some(1));
    assert_eq!(text[16].as_str(), Some("HEADER-ID"));
}

#[test]
fn build_stream_generate_model_header_prefers_image_edit_template_when_requested() {
    let storage_state = json!({
        "imageEditStreamGenerateTemplate": {
            "headers": {
                "x-goog-ext-525001261-jspb": "[1,null,null,null,\"56fdd199312815e2\",null,null,0,[4],null,null,2,null,null,1,null,\"EDIT-HEADER-ID\"]"
            }
        },
        "imageStreamGenerateTemplate": {
            "headers": {
                "x-goog-ext-525001261-jspb": "[1,null,null,null,\"56fdd199312815e2\",null,null,0,[4],null,null,2,null,null,1,null,\"IMAGE-HEADER-ID\"]"
            }
        }
    });

    let header = build_stream_generate_model_header_from_storage_state(
        &storage_state,
        GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX,
        true,
    )
    .unwrap();
    let parsed = serde_json::from_str::<Vec<Value>>(&header).unwrap();
    assert_eq!(parsed[16].as_str(), Some("EDIT-HEADER-ID"));

    let fallback_header = build_stream_generate_model_header_from_storage_state(
        &storage_state,
        GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX,
        false,
    )
    .unwrap();
    let fallback_parsed = serde_json::from_str::<Vec<Value>>(&fallback_header).unwrap();
    assert_eq!(fallback_parsed[16].as_str(), Some("IMAGE-HEADER-ID"));
}

#[test]
fn harvest_image_edit_template_locale_prefers_query_hl_then_accept_language() {
    let storage_state = json!({
        "imageEditStreamGenerateTemplate": {
            "url": "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate?bl=boq&f.sid=123&hl=zh-CN&_reqid=1&rt=c",
            "headers": {
                "accept-language": "en-US,en;q=0.9"
            }
        }
    });
    assert_eq!(
        harvest_image_edit_template_locale(&storage_state).as_deref(),
        Some("zh-CN")
    );

    let fallback = json!({
        "imageEditStreamGenerateTemplate": {
            "url": "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate?bl=boq&f.sid=123&_reqid=1&rt=c",
            "headers": {
                "accept-language": "ja-JP,ja;q=0.9"
            }
        }
    });
    assert_eq!(
        harvest_image_edit_template_locale(&fallback).as_deref(),
        Some("ja-JP")
    );
}

#[test]
fn build_text_batchexecute_model_header_matches_captured_tts_shapes() {
    assert_eq!(
        build_text_batchexecute_model_header(None, Some("HEADER-ID")),
        "[1,null,null,null,null,null,null,null,[4],null,null,null,null,null,1,null,\"HEADER-ID\"]"
    );
    assert_eq!(
            build_text_batchexecute_model_header(
                Some(GEMINI_CANVAS_TEXT_BOOTSTRAP_MODEL_ID),
                Some("HEADER-ID")
            ),
            "[1,null,null,null,\"fbb127bbb056c959\",null,null,null,[4],null,null,null,null,null,1,null,\"HEADER-ID\"]"
        );
    assert_eq!(
            build_text_batchexecute_model_header_variant(
                None,
                Some("HEADER-ID"),
                false,
                true
            ),
            "[1,null,null,null,\"\",null,null,null,[4],null,null,null,null,null,null,null,\"HEADER-ID\"]"
        );
}
