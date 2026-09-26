use super::followup_test_support::{make_payload, make_request};
use super::image_recovery::*;
use crate::protocol::canonical::{EndpointKind, ProtocolFamily};
use crate::protocol::gemini_canvas;
use serde_json::json;
use std::collections::HashMap;

fn assert_policy_action(
    actual: GeminiCanvasDirectHttpImageJsonAction,
    expected: GeminiCanvasDirectHttpImageJsonAction,
) {
    assert_eq!(actual, expected);
}

#[test]
fn json_policy_lane_matrix_never_enables_for_program_edit_or_non_image() {
    use gemini_canvas::GeminiCanvasMediaOperation::{Image, Music, Video};
    for adapter in [
        "gemini_canvas_compatible",
        "gemini_canvas_program_web_reverse_compatible",
    ] {
        for endpoint in [EndpointKind::ImagesGenerations, EndpointKind::ImagesEdits] {
            for operation in [Image, Music, Video] {
                for format in ["url", "b64_json"] {
                    for enabled in [false, true] {
                        let mut payload = make_payload(adapter, "https://gemini.google.com");
                        payload.extra_body = Some(HashMap::from([(
                            "imageJsonFallbackEnabled".to_string(),
                            json!(enabled.to_string()),
                        )]));
                        let mut req = make_request(ProtocolFamily::OpenAi, endpoint);
                        req.raw_body = json!({"response_format": format});
                        let policy = GeminiCanvasDirectHttpImageJsonPolicy::from_request(
                            &payload, &req, operation,
                        )
                        .unwrap();
                        let fallback = enabled
                            && adapter == "gemini_canvas_compatible"
                            && endpoint == EndpointKind::ImagesGenerations
                            && operation == Image;
                        let inline = fallback && format == "b64_json";
                        assert_eq!(policy.fallback_enabled, fallback);
                        assert_eq!(policy.inline_preferred, inline);
                        assert_eq!(
                            policy.initial_action(),
                            if inline {
                                GeminiCanvasDirectHttpImageJsonAction::TryJson
                            } else {
                                GeminiCanvasDirectHttpImageJsonAction::Skip
                            }
                        );
                        assert_eq!(
                            policy.on_stream_failure(),
                            if fallback && !inline {
                                GeminiCanvasDirectHttpImageJsonAction::TryJsonWithErrorContext {
                                    context_key: "stream_generate_failure",
                                }
                            } else {
                                GeminiCanvasDirectHttpImageJsonAction::ReturnOriginal
                            }
                        );
                        assert_eq!(
                            policy.on_materialize_failure(),
                            if inline {
                                GeminiCanvasDirectHttpImageJsonAction::TryJsonWithErrorContext {
                                    context_key: "image_json_recovery_failure",
                                }
                            } else {
                                GeminiCanvasDirectHttpImageJsonAction::ReturnOriginal
                            }
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn template_retry_matrix_never_replays_edits_even_without_async_marker() {
    for mode in [
        GeminiCanvasImageRecoveryMode::EditAsync,
        GeminiCanvasImageRecoveryMode::Generation,
    ] {
        for replay in [false, true] {
            for ready in [false, true] {
                let action = classify_gemini_canvas_image_template_retry(mode, replay, ready);
                match mode {
                    GeminiCanvasImageRecoveryMode::EditAsync => {
                        assert!(matches!(
                            action,
                            GeminiCanvasImageTemplateRetryAction::ReturnOriginalWithLog(_)
                        ));
                    }
                    GeminiCanvasImageRecoveryMode::Generation => {
                        assert_eq!(
                            action,
                            if replay {
                                GeminiCanvasImageTemplateRetryAction::RetryLegacyTemplate
                            } else {
                                GeminiCanvasImageTemplateRetryAction::ReturnOriginal
                            }
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn program_owned_image_b64_policy_disables_inline_json_prefill() {
    let mut payload = make_payload(
        "gemini_canvas_program_web_reverse_compatible",
        "https://gemini.google.com",
    );
    payload.extra_body = Some(HashMap::from([(
        "imageJsonFallbackEnabled".to_string(),
        json!("true"),
    )]));
    let mut req = make_request(ProtocolFamily::OpenAi, EndpointKind::ImagesGenerations);
    req.raw_body = json!({
        "response_format": "b64_json"
    });

    let policy = GeminiCanvasDirectHttpImageJsonPolicy::from_request(
        &payload,
        &req,
        gemini_canvas::GeminiCanvasMediaOperation::Image,
    )
    .expect("policy");

    assert_eq!(
        policy.initial_action(),
        GeminiCanvasDirectHttpImageJsonAction::Skip
    );
    assert_eq!(
        policy.on_stream_failure(),
        GeminiCanvasDirectHttpImageJsonAction::ReturnOriginal
    );
    assert_eq!(
        policy.on_materialize_failure(),
        GeminiCanvasDirectHttpImageJsonAction::ReturnOriginal
    );
}

#[test]
fn gemini_canvas_direct_http_image_json_policy_inline_prefill_and_recovery_contract() {
    let mut payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
    payload.extra_body = Some(HashMap::from([(
        "imageJsonFallbackEnabled".to_string(),
        json!("true"),
    )]));
    let mut req = make_request(ProtocolFamily::OpenAi, EndpointKind::ImagesGenerations);
    req.raw_body = json!({
        "response_format": "b64_json"
    });

    let mut policy = GeminiCanvasDirectHttpImageJsonPolicy::from_request(
        &payload,
        &req,
        gemini_canvas::GeminiCanvasMediaOperation::Image,
    )
    .expect("policy");

    assert_policy_action(
        policy.initial_action(),
        GeminiCanvasDirectHttpImageJsonAction::TryJson,
    );

    policy.note_prefill_failure("prefill failed".to_string());
    assert_eq!(
        policy.on_stream_failure(),
        GeminiCanvasDirectHttpImageJsonAction::ReturnOriginalWithSummary {
            context_key: "image_json_prefill_failure",
            summary: "prefill failed".to_string(),
        }
    );
}

#[test]
fn gemini_canvas_direct_http_image_json_policy_stream_fallback_only_for_non_inline_mode() {
    let mut payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
    payload.extra_body = Some(HashMap::from([(
        "imageJsonFallbackEnabled".to_string(),
        json!("true"),
    )]));
    let mut req = make_request(ProtocolFamily::OpenAi, EndpointKind::ImagesGenerations);
    req.raw_body = json!({
        "response_format": "url"
    });

    let policy = GeminiCanvasDirectHttpImageJsonPolicy::from_request(
        &payload,
        &req,
        gemini_canvas::GeminiCanvasMediaOperation::Image,
    )
    .expect("policy");

    assert_policy_action(
        policy.initial_action(),
        GeminiCanvasDirectHttpImageJsonAction::Skip,
    );
    assert_eq!(
        policy.on_stream_failure(),
        GeminiCanvasDirectHttpImageJsonAction::TryJsonWithErrorContext {
            context_key: "stream_generate_failure",
        }
    );
}

#[test]
fn gemini_canvas_direct_http_image_json_policy_disables_legacy_json_for_edits() {
    let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
    let mut req = make_request(ProtocolFamily::OpenAi, EndpointKind::ImagesEdits);
    req.raw_body = json!({
        "response_format": "b64_json"
    });

    let policy = GeminiCanvasDirectHttpImageJsonPolicy::from_request(
        &payload,
        &req,
        gemini_canvas::GeminiCanvasMediaOperation::Image,
    )
    .expect("policy");

    assert_policy_action(
        policy.initial_action(),
        GeminiCanvasDirectHttpImageJsonAction::Skip,
    );
    assert_policy_action(
        policy.on_stream_failure(),
        GeminiCanvasDirectHttpImageJsonAction::ReturnOriginal,
    );
}

#[test]
fn gemini_canvas_direct_http_image_json_policy_materialize_recovery_matches_inline_mode() {
    let mut payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
    payload.extra_body = Some(HashMap::from([(
        "imageJsonFallbackEnabled".to_string(),
        json!("true"),
    )]));

    let mut inline_req = make_request(ProtocolFamily::OpenAi, EndpointKind::ImagesGenerations);
    inline_req.raw_body = json!({
        "response_format": "b64_json"
    });
    let inline_policy = GeminiCanvasDirectHttpImageJsonPolicy::from_request(
        &payload,
        &inline_req,
        gemini_canvas::GeminiCanvasMediaOperation::Image,
    )
    .expect("inline policy");
    assert_eq!(
        inline_policy.on_materialize_failure(),
        GeminiCanvasDirectHttpImageJsonAction::TryJsonWithErrorContext {
            context_key: "image_json_recovery_failure",
        }
    );

    let mut url_req = make_request(ProtocolFamily::OpenAi, EndpointKind::ImagesGenerations);
    url_req.raw_body = json!({
        "response_format": "url"
    });
    let url_policy = GeminiCanvasDirectHttpImageJsonPolicy::from_request(
        &payload,
        &url_req,
        gemini_canvas::GeminiCanvasMediaOperation::Image,
    )
    .expect("url policy");
    assert_policy_action(
        url_policy.on_materialize_failure(),
        GeminiCanvasDirectHttpImageJsonAction::ReturnOriginal,
    );
}

#[test]
fn resolve_gemini_canvas_image_recovery_mode_matches_endpoint_kind() {
    assert_eq!(
        resolve_gemini_canvas_image_recovery_mode(EndpointKind::ImagesEdits),
        GeminiCanvasImageRecoveryMode::EditAsync
    );
    assert_eq!(
        resolve_gemini_canvas_image_recovery_mode(EndpointKind::ImagesGenerations),
        GeminiCanvasImageRecoveryMode::Generation
    );
}

#[test]
fn classify_gemini_canvas_image_template_retry_matches_edit_and_non_edit_contracts() {
    assert_eq!(
        classify_gemini_canvas_image_template_retry(
            GeminiCanvasImageRecoveryMode::EditAsync,
            true,
            true,
        ),
        GeminiCanvasImageTemplateRetryAction::ReturnOriginalWithLog(
            "gemini canvas direct HTTP image-edit template lane reached async-followup-ready; suppressing legacy heavy retry",
        )
    );
    assert_eq!(
        classify_gemini_canvas_image_template_retry(
            GeminiCanvasImageRecoveryMode::EditAsync,
            true,
            false,
        ),
        GeminiCanvasImageTemplateRetryAction::ReturnOriginalWithLog(
            "gemini canvas direct HTTP image-edit template lane ended without a usable asset; suppressing legacy heavy retry to preserve caller-visible transport budget",
        )
    );
    assert_eq!(
        classify_gemini_canvas_image_template_retry(
            GeminiCanvasImageRecoveryMode::Generation,
            false,
            false,
        ),
        GeminiCanvasImageTemplateRetryAction::ReturnOriginal
    );
    assert_eq!(
        classify_gemini_canvas_image_template_retry(
            GeminiCanvasImageRecoveryMode::Generation,
            true,
            false,
        ),
        GeminiCanvasImageTemplateRetryAction::RetryLegacyTemplate
    );
}

#[test]
fn build_gemini_canvas_image_recovery_strategy_matches_lane_contracts() {
    let edit_ready = build_gemini_canvas_image_recovery_strategy(
        EndpointKind::ImagesEdits,
        true,
        concat!(
            ")]}'\n\n",
            "126\n",
            "[[\"wrb.fr\",null,\"[null,[null,\\\"r_async123\\\"],{\\\"18\\\":\\\"r_async123\\\",\\\"21\\\":[\\\"token\\\"],\\\"44\\\":true}]\"]]\n"
        ),
    );
    assert_eq!(edit_ready.mode, GeminiCanvasImageRecoveryMode::EditAsync);
    assert_eq!(
        edit_ready.template_retry_action,
        GeminiCanvasImageTemplateRetryAction::ReturnOriginalWithLog(
            "gemini canvas direct HTTP image-edit template lane reached async-followup-ready; suppressing legacy heavy retry",
        )
    );

    let generation_retry = build_gemini_canvas_image_recovery_strategy(
        EndpointKind::ImagesGenerations,
        true,
        "plain body without async marker",
    );
    assert_eq!(
        generation_retry.mode,
        GeminiCanvasImageRecoveryMode::Generation
    );
    assert_eq!(
        generation_retry.template_retry_action,
        GeminiCanvasImageTemplateRetryAction::RetryLegacyTemplate
    );
}
