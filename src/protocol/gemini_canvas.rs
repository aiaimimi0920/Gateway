use base64::Engine;
use image::codecs::jpeg::JpegEncoder;
use image::ExtendedColorType;
#[cfg(test)]
use image::ImageEncoder;
use regex::Regex;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use url::Url;

use crate::error::GatewayError;
use crate::protocol::canonical::{
    CanonicalMessage, CanonicalRelayRequest, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
};
use crate::protocol::gemini::canvas_web_reverse as gemini_canvas_modular;
use crate::protocol::gemini_business;
use crate::protocol::gemini_web;
use crate::routing::candidate::ProviderAccountPayload;

pub const GEMINI_CANVAS_DEFAULT_MODEL: &str = "gemini-2.5-flash-image-preview";
pub const GEMINI_CANVAS_DEFAULT_TEXT_MODEL: &str = "gemini-3-flash-preview";
pub const GEMINI_CANVAS_DEFAULT_TTS_MODEL: &str = "gemini-2.5-flash-preview-tts";
pub const GEMINI_CANVAS_DEFAULT_API_BASE_URL: &str =
    "https://generativelanguage.googleapis.com/v1beta";
pub const GEMINI_CANVAS_DIRECT_HTTP_IMAGE_API_BASE_URL: &str =
    "https://geminiweb-pa.clients6.google.com/v1beta";
pub const GEMINI_CANVAS_DEFAULT_SHARE_ID: &str = "fe24c455a570";
pub const GEMINI_CANVAS_DEFAULT_SHARE_URL: &str = "https://gemini.google.com/share/fe24c455a570";
pub const GEMINI_CANVAS_IMAGEN_4_MODEL: &str = "imagen-4.0-generate-001";
pub const GEMINI_CANVAS_IMAGEN_3_MODEL: &str = "imagen-3.0-generate-002";
pub const GEMINI_CANVAS_IMAGEN_3_LEGACY_MODEL: &str = "imagen-3.0-generate-001";
pub const GEMINI_CANVAS_OFFICIAL_IMAGE_MODEL: &str = "gemini-2.5-flash-image";
pub const GEMINI_CANVAS_OFFICIAL_IMAGE_MODEL_PREVIEW: &str = "gemini-3.1-flash-image-preview";
pub const GEMINI_CANVAS_OFFICIAL_MUSIC_MODEL: &str = "lyria-realtime-exp";
pub const GEMINI_CANVAS_OFFICIAL_VIDEO_MODEL: &str = "veo-3.1-generate-preview";
pub const GEMINI_CANVAS_OFFICIAL_TTS_MODEL: &str = "gemini-2.5-flash-preview-tts";
pub const GEMINI_25_FLASH_IMAGE_PREVIEW_MODEL: &str = "gemini-2.5-flash-image-preview";
pub const GEMINI_25_FLASH_IMAGE_MODEL: &str = "gemini-2.5-flash-image";
pub const GEMINI_31_FLASH_IMAGE_PREVIEW_MODEL: &str = "gemini-3.1-flash-image-preview";
pub const GEMINI_CANVAS_MUSIC_PREVIEW_MODEL: &str = "gemini-canvas-music-preview";
pub const GEMINI_CANVAS_VIDEO_PREVIEW_MODEL: &str = "gemini-canvas-video-preview";
pub const GEMINI_CANVAS_TEXT_SELECTED_MODEL_HEADER_ID: &str = "56fdd199312815e2";
pub const GEMINI_CANVAS_TEXT_LAST_SELECTED_MODE_ID: &str = "fbb127bbb056c959";
pub const GEMINI_CANVAS_TEXT_STREAM_GENERATE_MODEL_HEADER: &str =
    r#"[1,null,null,null,"fbb127bbb056c959",null,null,0,[4],null,null,1,null,null,1]"#;
pub const GEMINI_CANVAS_IMAGE_STREAM_GENERATE_MODEL_HEADER: &str =
    r#"[1,null,null,null,"56fdd199312815e2",null,null,0,[4],null,null,1]"#;
pub const GEMINI_CANVAS_MEDIA_STREAM_GENERATE_MODEL_HEADER: &str =
    r#"[1,null,null,null,"56fdd199312815e2",null,null,0,[4],null,null,2]"#;
pub const GEMINI_CANVAS_TEXT_STREAM_GENERATE_MODEL_HEADER_2: &str = "[0]";
pub const GEMINI_CANVAS_TEXT_STREAM_GENERATE_MODEL_HEADER_3: &str = "[0]";
pub const GEMINI_CANVAS_TEXT_PREFLIGHT_MODEL_HEADER_2: &str = "[0]";

pub fn unsupported_request_plan_error() -> GatewayError {
    GatewayError::bad_request(
        "Gemini Canvas adapters use dedicated browser-backed reverse-web text/TTS/media send paths and are not supported by the generic request planner.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("unsupported_gemini_canvas_endpoint")
}

pub fn unsupported_media_adapter_endpoint_error(provider: &str) -> GatewayError {
    GatewayError::bad_request(
        "Gemini Canvas adapters currently support /v1/images/generations, /v1/images/edits, /v1/music/generations, and /v1/videos/generations.",
    )
    .with_provider(provider)
    .with_code("unsupported_gemini_canvas_endpoint")
}

pub fn unsupported_modular_endpoint_error(provider: &str) -> GatewayError {
    GatewayError::bad_request(
        "Gemini Canvas modular browser relay currently supports /v1/images/generations, /v1/music/generations, /v1/videos/generations, and /v1/audio/speech.",
    )
    .with_provider(provider)
    .with_code("unsupported_gemini_canvas_modular_endpoint")
}

pub fn unsupported_image_count_error(provider: &str) -> GatewayError {
    GatewayError::bad_request(
        "Gemini Canvas image generation currently supports only n=1 requests.",
    )
    .with_provider(provider)
    .with_code("unsupported_gemini_canvas_image_count")
}

pub fn unsupported_modular_image_count_error(provider: &str) -> GatewayError {
    GatewayError::bad_request(
        "Gemini Canvas browser relay image generation currently supports only n=1 requests.",
    )
    .with_provider(provider)
    .with_code("unsupported_gemini_canvas_modular_image_count")
}

pub fn unsupported_image_edit_count_error(provider: &str) -> GatewayError {
    GatewayError::bad_request("Gemini Canvas image edits currently support only n=1 requests.")
        .with_provider(provider)
        .with_code("unsupported_gemini_canvas_image_edit_count")
}

pub fn unsupported_modular_image_edits_error(provider: &str) -> GatewayError {
    GatewayError::bad_request(
        "Gemini Canvas modular browser relay does not implement image edits yet. Keep using the legacy mixed lane until the true browser-owned edit flow is split out.",
    )
    .with_provider(provider)
    .with_code("unsupported_gemini_canvas_modular_image_edits")
}

pub const GEMINI_CANVAS_TEXT_PREFLIGHT_HEADER_ID_DEFAULT: &str =
    "69BD1D7B-C622-42E4-AB1E-B6748B37B679";
pub const GEMINI_CANVAS_TEXT_MODE_SELECTION_MODEL_HEADER: &str =
    r#"[1,null,null,null,null,null,null,null,[4],null,null,null,null,null,1]"#;
pub const GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER_SHORT: &str =
    r#"[1,null,null,null,null,null,null,null,[4]]"#;
pub const GEMINI_CANVAS_TEXT_PREFLIGHT_NULL_MODEL_HEADER: &str =
    r#"[1,null,null,null,null,null,null,null,[4],null,null,null,null,null,1]"#;
pub const GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID: &str = "L5adhe";
pub const GEMINI_CANVAS_MEDIA_OPERATION_SELECTION_RPCID: &str = "qpEbW";
pub const GEMINI_CANVAS_TEXT_BOOTSTRAP_MODEL_HEADER: &str =
    r#"[1,null,null,null,"fbb127bbb056c959",null,null,null,[4],null,null,null,null,null,1]"#;
pub const GEMINI_CANVAS_TEXT_BOOTSTRAP_MODEL_ID: &str = "fbb127bbb056c959";
pub const GEMINI_CANVAS_TEXT_BOOTSTRAP_RPCID: &str = "aPya6c";
pub const GEMINI_CANVAS_TEXT_STATE_PREFLIGHT_RPCID: &str = "ESY5D";
pub const GEMINI_CANVAS_IMAGE_STATE_KEYS_PREFLIGHT_KEYS: &[&str] = &[
    "adaptive_device_responses_enabled",
    "advanced_mode_theme_override_triggered",
    "advanced_zs_upsell_dismissal_count",
    "advanced_zs_upsell_last_dismissed",
    "ai_transparency_notice_dismissed",
    "audio_overview_discovery_dismissal_count",
    "audio_overview_discovery_last_dismissed",
    "bard_in_chrome_link_sharing_enabled",
    "bard_sticky_mode_disabled_count",
    "canvas_create_discovery_tooltip_seen_count",
    "combined_files_button_tag_seen_count",
    "indigo_banner_explicit_dismissal_count",
    "indigo_banner_impression_count",
    "indigo_banner_last_seen_sec",
    "current_popup_id",
    "deep_research_has_seen_file_upload_tooltip",
    "deep_research_model_update_disclaimer_display_count",
    "default_bot_id",
    "disabled_discovery_card_feature_ids",
    "disabled_model_discovery_tooltip_feature_ids",
    "disabled_mode_disclaimers",
    "disabled_new_model_badge_mode_ids",
    "disabled_settings_discovery_tooltip_feature_ids",
    "disablement_disclaimer_last_dismissed_sec",
    "disable_advanced_beta_dialog",
    "disable_advanced_beta_non_en_banner",
    "disable_advanced_resubscribe_ui",
    "disable_at_mentions_discovery_tooltip",
    "disable_autorun_fact_check_u18",
    "disable_bot_create_tips_card",
    "disable_bot_docs_in_gems_disclaimer",
    "disable_bot_onboarding_dialog",
    "disable_bot_save_reminder_tips_card",
    "disable_bot_send_prompt_tips_card",
    "disable_bot_shared_in_drive_disclaimer",
    "disable_bot_try_create_tips_card",
    "disable_colab_tooltip",
    "disable_collapsed_tool_menu_tooltip",
    "disable_continue_discovery_tooltip",
    "disable_debug_info_moved_tooltip_v2",
    "disable_enterprise_mode_dialog",
    "disable_export_python_tooltip",
    "disable_extensions_discovery_dialog",
    "disable_extension_one_time_badge",
    "disable_fact_check_tooltip_v2",
    "disable_free_file_upload_tips_card",
    "disable_generated_image_download_dialog",
    "disable_get_app_banner",
    "disable_get_app_desktop_dialog",
    "disable_googler_in_enterprise_mode",
    "disable_human_review_disclosure",
    "disable_ice_open_vega_editor_tooltip",
    "disable_images_new_badge",
    "disable_image_upload_tooltip",
    "disable_legal_concern_tooltip",
    "disable_llm_history_import_disclaimer",
    "disable_location_popup",
    "disable_memory_discovery",
    "disable_memory_extraction_discovery",
    "disable_new_conversation_dialog",
    "disable_onboarding_experience",
    "disable_personal_context_tooltip",
    "disable_photos_upload_disclaimer",
    "disable_power_up_intro_tooltip",
    "disable_scheduled_actions_mobile_notification_snackbar",
    "disable_storybook_listen_button_tooltip",
    "disable_streaming_settings_tooltip",
    "disable_take_control_disclaimer",
    "disable_teens_only_english_language_dialog",
    "disable_temp_chat_soft_badge",
    "disable_tier1_rebranding_tooltip",
    "disable_try_advanced_mode_dialog",
    "enable_advanced_beta_mode",
    "enable_advanced_mode",
    "enable_googler_in_enterprise_mode",
    "enable_memory",
    "enable_memory_extraction",
    "enable_personal_context",
    "enable_personal_context_gemini",
    "enable_personal_context_gemini_using_health",
    "enable_personal_context_gemini_using_photos",
    "enable_personal_context_gemini_using_workspace",
    "enable_personal_context_search",
    "enable_personal_context_youtube",
    "enable_token_streaming",
    "enforce_default_to_fast_version",
    "mayo_discovery_banner_dismissal_count",
    "mayo_discovery_banner_last_dismissed_sec",
    "get_app_banner_ack_count",
    "get_app_banner_seen_count",
    "get_app_mobile_dialog_ack_count",
    "guided_learning_banner_dismissal_count",
    "guided_learning_banner_last_dismissed",
    "has_accepted_agent_mode_fre_disclaimer",
    "has_accepted_gemini_notebooks_eea_disclaimer",
    "has_personalization_profile_completion_notice_shown",
    "has_received_streaming_response",
    "has_seen_agent_mode_tooltip",
    "has_seen_bespoke_tooltip",
    "has_seen_bracket_discovery_banner",
    "has_seen_deepthink_mustard_tooltip",
    "has_seen_deepthink_v2_tooltip",
    "has_seen_deep_think_tooltip",
    "has_seen_first_youtube_video_disclaimer",
    "has_seen_ggo_tooltip",
    "has_seen_image_grams_discovery_banner",
    "has_seen_image_preview_in_input_area_tooltip",
    "has_seen_kallo_discovery_banner",
    "has_seen_kallo_fs_tooltip",
    "has_seen_kallo_full_song_discovery_banner",
    "has_seen_kallo_tooltip",
    "has_seen_likeness_discovery_banner",
    "has_seen_llm_history_import_page",
    "has_seen_model_picker_in_input_area_tooltip",
    "has_seen_notebooks_splash",
    "has_seen_omni_discovery_banner",
    "has_seen_veograms_discovery_banner",
    "has_seen_veo_freemium_discovery_banner",
    "images_new_badge_impression_count",
    "is_imported_chats_panel_open_by_default",
    "jumpstart_onboarding_dismissal_count",
    "last_dismissed_deep_research_implicit_invite",
    "last_dismissed_discovery_feature_implicit_invites",
    "last_dismissed_immersives_canvas_implicit_invite",
    "last_dismissed_immersive_share_disclaimer_sec",
    "last_dismissed_strike_timestamp_sec",
    "last_dismissed_zs_student_aip_banner_sec",
    "last_get_app_banner_ack_timestamp_sec",
    "last_get_app_mobile_dialog_ack_timestamp_sec",
    "last_human_review_disclosure_ack",
    "last_selected_mode_id_in_embedded",
    "last_selected_mode_id_on_web",
    "last_two_up_activation_timestamp_sec",
    "last_winter_olympics_interaction_timestamp_sec",
    "ecxo_discovery_banner_dismissal_count",
    "ecxo_discovery_banner_last_dismissed_sec",
    "memory_extracted_greeting_name",
    "mini_gemini_tos_closed",
    "mode_switcher_soft_badge_disabled_ids",
    "mode_switcher_soft_badge_seen_count",
    "opt_out_your_day",
    "personalization_first_party_onboarding_cross_surface_clicked",
    "personalization_first_party_onboarding_cross_surface_seen_count",
    "personalization_one_p_discovery_card_seen_count",
    "personalization_one_p_discovery_last_consented",
    "personalization_profile_latency_mask_shown_count",
    "personalization_zero_state_card_last_interacted",
    "personalization_zero_state_card_seen_count",
    "popup_zs_visits_cooldown",
    "require_reconsent_setting_for_personalization_banner_seen_count",
    "settings_menu_soft_badge_disabled_ids",
    "show_debug_info",
    "side_nav_open_by_default",
    "student_verification_dismissal_count",
    "student_verification_last_dismissed",
    "task_viewer_cc_banner_dismissed_count",
    "task_viewer_cc_banner_dismissed_time_sec",
    "tool_menu_new_badge_disabled_ids",
    "tool_menu_new_badge_impression_counts",
    "tool_menu_soft_badge_disabled_ids",
    "tool_menu_soft_badge_impression_counts",
    "upload_disclaimer_last_consent_time_sec",
    "viewed_student_aip_upsell_campaign_ids",
    "voice_language",
    "voice_name",
    "web_and_app_activity_enabled",
    "wellbeing_nudge_notice_last_dismissed_sec",
    "zs_student_aip_banner_dismissal_count",
];
pub const GEMINI_CANVAS_TTS_TRIGGER_RPCID: &str = "PCck7e";
pub const GEMINI_CANVAS_TTS_EXPORT_RPCID: &str = "XqA3Ic";
pub const GEMINI_CANVAS_TTS_TRIGGER_MODEL_HEADER: &str =
    GEMINI_CANVAS_TEXT_MODE_SELECTION_MODEL_HEADER;
pub const GEMINI_CANVAS_TTS_FOLLOWUP_MODEL_HEADER: &str = GEMINI_CANVAS_TEXT_BOOTSTRAP_MODEL_HEADER;
pub const GEMINI_CANVAS_BROWSER_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36";
pub const GEMINI_CANVAS_BROWSER_SEC_CH_UA: &str =
    "\"Not.A/Brand\";v=\"8\", \"Chromium\";v=\"147\", \"Google Chrome\";v=\"147\"";
pub const GEMINI_CANVAS_BROWSER_SEC_CH_UA_FULL_VERSION: &str = "\"147.0.7727.102\"";
pub const GEMINI_CANVAS_BROWSER_SEC_CH_UA_FULL_VERSION_LIST: &str =
    "\"Not.A/Brand\";v=\"8.0.0.0\", \"Chromium\";v=\"147.0.7727.102\", \"Google Chrome\";v=\"147.0.7727.102\"";
pub const GEMINI_CANVAS_BROWSER_SEC_CH_UA_PLATFORM_VERSION: &str = "\"19.0.0\"";
pub const GEMINI_CANVAS_BROWSER_CHANNEL: &str = "stable";
pub const GEMINI_CANVAS_BROWSER_COPYRIGHT: &str = "Copyright 2026 Google LLC. All Rights reserved.";
pub const GEMINI_CANVAS_BROWSER_VALIDATION: &str = "B2gM+WTW2xHE15IAjh8nDoMc5x0=";
pub const GEMINI_CANVAS_BROWSER_YEAR: &str = "2026";
pub const GEMINI_CANVAS_TEXT_STREAM_GENERATE_TEXT_MODE_INDEX: i64 = 2;
pub const GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX: i64 = 14;
pub const GEMINI_CANVAS_STREAM_GENERATE_VIDEO_MODE_INDEX: i64 = 11;
pub const GEMINI_CANVAS_STREAM_GENERATE_MUSIC_MODE_INDEX: i64 = 21;
pub const GEMINI_CANVAS_TEXT_STREAM_GENERATE_OPAQUE_STATE: &str = "!3d6l3rrNAAZP59t2b-dCepbdr7lrV7I7AEABEArZ1E6mVulXqJyNmu7tQE8dXsGnls9Ib186UKmMmsN3M6iDKdF36NuEfGAzu7SSeXDZnGimd_-6wRyT7z0kAgAAAKVSAAAAB2gBB34AQtPEpVAvtXlGIjeoltWsDc4hc1ak5cDRmJWztuTsxu5LqRW6miSRBvB_GASrF3HrQKGki8DzdBmGWijcXqXYc1CCTpkDigAJGEvW_R-LQiO6ghhZ6HoI_SklGPCqKQ5eCpvWSkaxc4URs07dZPo9qWCujvSSEf51EI7dZgo_oD0Vr9TVm_4f5p5kDX_x1FxfbWYmwKH3obAu1QPTLWAUXKKvq6z_M-_AKbOiV27bUF_SPbZpyUSdjppJr_EMKrs-1Y2J98uIHefQsaxDZB-lal_ymc_kAfiV5NIBRuoAFg4VJQIp9ye0SRYhOxOBEN6PJzq1NnXJ3HrjGgpL-QYSgi1Ei44IszZO9jAtbAjEWCshwtU0wz4jgCPVUC9QVMpT-JL4V9XyzWGeoih4uWUZ05r-M0uqNu0CtHjLA3K9O_m50Lc8mRYy5QXn-HZn77mR5HwK4TqckmXc69wO8dxtYjAgegO7--6iabjRjIVmWrWGc6g4T_CEI3BdVnYwps8m1DgGZu5OpOYb-eTUCZxD_nPqldDRHmAjzFtWdp6iHn1sqP-5DssbH0vzXhKtJngDjv44qNgkJ6wgAHAWXaoGcEJ9eTe0WDTFe4Q1g0EtJgTF6PryperdCJ69UEmTPBBKIgPGD9g1N-ZOwU9Os57HZ6eh3g2SGXeaTwZSBrv3S_S9FKhOCWUCGZlNZ4uO1Cra8tBk_OVX1RBd9LPtOs5xpI_hDXoWx_xYjdFeNtTY0Uv05j6KWhBEmK4iZc1zlrQAD8o23ipewQ4uu0g3Z_rRHRF8Yik5BX82aYnob0X-pdPD_Ljae3xM5zQfMcWSywfIBjacO5vpijrBqoUxSU79YTxCnCrcVSq3XIACOkrjTYeYDpVaXJ76vVAL-ypAd2Yot93LY14Q4ZQko68c04QiXgq6os9VGfAFW3HF2avzh3JdV0owi0bblLpowUoYbHreCgCAM451XaP95L1LkeY4-8qTx3wMOTGXIswfueYApyPjL7h5xdvnG2JNfHzEpaOnoGOk_mouN-HCBstTlFxVEnsaD5c228oWeZjWjPmdhEHEs9mAzJliQpzRIb64OrzLayucOQzcTqnUdIkDCjbRBdUWEWvXDDwN6-Ic_L4Eu-VEuGOrIugaH8PLav84Wc18Zu5ynxlwcxCrEf73paruEQlbh5lHY_KSYCcORBufQk86DG64s7rnGytqmnyqJgdNd6ofEaCC_LUJse-jRPwoBxdVvEff9KJRHSHVg2DA6HGyWoReXy1F3FReaaZ6klibCyf0V4DZqyuuLlbJpYSyyQ";
pub const GEMINI_CANVAS_IMAGE_STREAM_GENERATE_OPAQUE_STATE: &str = "!HR6lHnrNAAam_pEchv5CcZYkV42Gumg7AEABEArZ1I3dRaayZxTLeir-7bK5s0rgc7FM9dzGNYIw2g-A1fwIXW7j_xqkjFDSK1C2RGqCvS3GPMeNf3lUN3lqAgAAAItSAAAAB2gBB34ARLQ7YAVZNjXxOktL0eGGc68TTGxav7sjzealsHZNdzN9zOoB2z9_kMSsERCH4JGdKQuJuKVL1QkRTnB0nFBuWGaLOJNfmQXhNOyJCG5nUhLyTKEoISGomINTM5vbrT0uydMBkrZ7iWayFN3Qms9iuKtyd_izoJk2yf8nO37j0lghi3_XJDlncs-b-M3SEZHQ4XY0WLmq0CILaaVty61hpcwiQ14fqLniM6d-4Piq2W6ExwCX_J_bAEwpIFhpKpO9redTq24qOdn2QyfaJvL-CQFSsc1uzy15n2G9-5YOVjF4xfAXIFVxxGV6BYSVsan_g4kxUSJVZHzydohSx5grVAdH9U9nsrmwlP50EiRIV9ZR-v6DAlq2neZ521wWnDWXnG8NB9Ro-pim1cuQDdTel2aogEp13y4I0SlkDCvc8141qHaDo2cziul5y2oDe9uDj3Oz4133HRU5W5bQvL4OYS9EjHIWfp8YZiUYQBoGi7li6EYrKghSTEERv2wowa6qtgLhetDM94OINq0b0D-W16goRKRQutvlr7y7r9jm3sV0TnnPUsnupIHUpFcHTu7H12baIXd1x6xX3nh-TYelM-8MDnqCk5h-oE5ZCRmhiWtC2zY-TRSIhWLPuE4qipb3Pqne4schZvXrdf6u19Gv_803hKZ-h1WRE2KoPms6bkFI6rI0_WXI-awOJi8nX8x2Lir4B6FJ-E20qvdYLMdLACbDgpFvkE4TQWeoFRNeWtbLtyhuIj0yh_eOL6tsuI4xVJiFWlZwgivKkXQE68Kpndg-QqrGNDxqmfofwCSwayS0A9pgkm8qHqHJaWDPjgDJ-aFkgil6LSJMR75FsAq302eOD6GqZjoKuZ1yKtqjSO6O-rQ4iBVJi530e4UTMMlUERZGp0c4pF_ngETGaPLtpYw3r-lSjngw_3GWZDnQhEqMkSmaIAGOns1vYb7eyGFC8CZjquSxU9s7pHfnQmwy3oGIsTMihHVKigYl7vEsDLTraROCB1d6CdR3uMEKLcsLuZhjMS41dwTyH6BV_Nlx3KaCzrq8Sb1Ey5MwAe6cp_gNI5QlZuo9IGy0haMvRHKb4S-zSHr7kbqXQVvgfi8mid4jnxpfzCgoL54L1UrTr8TZnNHUxHOhWBX0yitX5brZwX3GMEY6YwTYvM_SBmHwfNMQYTrlesAmN_tFpj3rECiGX7gayWNC3g9BPC_0F8eK9OmopJWxzhti-n62FVpI1_xYQbNMvY-kQZG_dXvFB1eZL8bSY0zIFyX0M8eLucB9jP1k-2-NXnl8201IZz7NJVCQcIt6fwxXPGP7PuqPtN22hADWtmdTAxdn-hngyjSYssCsRFyAuVJ00Z3gSXVhXBvZROgaV1g27sCAvThX_YXrB2g3J-jlyYdWvn7CP-uTz6qrieGBZvukueGJAFHfZZZCmvzQ40a0yjQ4RhhE6_iTnOcTFapbwG-1Q3DbTDSfeZm9oRImjqCo4-qRUdtuf3-YKFFcsF1omS-zwvex1Lekeim4b8QB4uvtBkbMiRxk7lPMULg0cDrAgX9wrD2suGlDKjVeq-l0x-CWJV6Zarp-5jcPd75Jd1g-GAnpV1kdivYoAXaQDfi2ywWDrk007bW3bUrF2kHqgw_5zCNWSqiNrAI7iqGMi6LhKhS6rLQhx7yLHtITifC5bp16DXDLVQYQgj72CXfsH2z2IhuNeARonRrPst0nhtvUqK7HPa2-yvVe5YXs_feWXH1uxg0I8CDlYYwr4aKFhD5IxOMARhKASsQCEuqubWCukbQTbAS6RVcTOLkIIOuxpfDT-N_kTXR7X6DZ1g2xXUsHgezc-q72orfM4VKzt0xb_-mR7WKpw6-X_2TYbUBBClAduMS5cap0IiJ9icRL9C6sIrgLEG5YKntmVy8VyivTHWSe9mU7tDgZdHWRV4XpdqWSV9mpIpk_t2RxwTxaFNS18y3rAd8xtg9j9R6kYO8DMBoT_xZ-OVcH0Ci4vv1PEUnps9LCaPUxw6zeAtRQ3hxwxctyPQqS9R7EmpTExYVqCLYT9-ei3UG6MhjvCxhHazBd5oQf7p-X26uWSzrTGQBZcZuolrtCSjcCl-EMsro";
pub const GEMINI_CANVAS_MEDIA_STREAM_GENERATE_OPAQUE_STATE: &str = "!Dg2lDWnNAAYJtcN5KbVC328qU_Q5ruU7AEABEArZ1Cwei9-8bhDJn9NMsfkjpjQK6wlauEUXRzz9e2dfpmL5RNqEAtT7AeVN0Ev63OcrGXQCG-R-m_mksVGWAgAAAE5SAAAABmgBB34AQT_b06Lmmfrow-NTYh5ADoC-tWvd78HEOmIEXBzbMRHct-EqxKzOG2dHZXbMTZJwY5DDnulwx8S1o6BkHi313t6JmQNf3b33zoDItu9Fk66t400c_goVafOG8CgxIFshL0-jxGo2oFK-eqyiLGHU98Rgjg2sHAgU08-dJnOmxuN-wPgGJDruocA1tG6WTLyldxu-FH0cY1T8Guylb0zOtxApJ7-qwk6MVdR2tE5WFhV__Vzt5HWQzHkI26KW39EmoBhTaWvYP7cqlhYlDzPeBjoixjB7dsV32gnr-0Rxg7n-2lg3odWTFbRzFBnmeZaeTRxqSzAbaomJopezgnO18pBCTR6Tapg5Z879qLJFjUHlA3aBoW15IjXgr3vOi322P_Epg4LgNMpW9wXW0QEYDxYW5ZLl9aFPoWA_l3OtvBfFY2urYkdeJf-oq3Ma-rctylog1IsMwlQnRy2Fs9EOvTCjv8sPSmLTTy-TUo0trP9tK3SdV7FYAFc4Mj6soTV1Ng3Vvq8cQTf6aw9YiqYMHlLb8oKR2xMu8yk5Zv4s92hFsnU4B4fQZcpkbpdio1uSzn1lwRG0oVKNIlOaLMwtLxRFKYF0H_COGA4yvTvf0N5BMn-yUes2x_GhwQfOl-uj40QKtE2dStgoS9UPWWOvkBwPQMbds0ZBIoluXZliIVNg2W70NVBL8zltkJLDsJo_nQgkalYaKzDweMLqnS5TKdiaXJNa2MHSIXnPCq3I_NVIKTX8mR9sngv7wk94GF5ByTFp_NYjIbwbwwItA4aoIXxr_eGy406bi3_zZkRpvPvpaXlaXxrqaXbC5nRK2pvb77kPGH8lgQVe68b7FfEBajo0y5o3D2tEzNx6TzhbWZpohsYZJ_oaAW3kBWuc1zlUZFp21rTxe_dmq_AVeWMB0717L73b10JQck40hYknd56bCWLqrtoo23uXMn6M2Yb_vHaGgZAkIBctwG6W3-r6mZnALCIisGwJYuMQbvxTx2xcekxJTFyBChucADhapyA_AOAR9PfZlUIFsQfSdnbS8JuqfVFeZhk_1LgAUP-Vul3RBJkM4HE60kIBRhaf9pVZhQCNBI1SvsqtH8r_3q9m7Cx58fI2eIf86PbnvZ4XhRxZv_zQKBQYi3-XcJQOeIkKjcJB8Ni-_beVPA_u_it-9lwr_I7AaAndCrr8GsJaBSU0P7_bNV68qSU9FaoX4lOyeOCjq9QDxox5nKWHiFn-5HZOBM4";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiCanvasImageEditUpload {
    pub mime_type: String,
    pub bytes: Vec<u8>,
    pub file_name: String,
    pub source_mime_type: String,
    pub source_bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiCanvasUploadedFileRef {
    pub resource_path: String,
    pub mime_type: String,
    pub file_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiCanvasStreamGenerateSeed {
    pub opaque_state: Option<String>,
    pub request_hex: Option<String>,
    pub request_uuid: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiCanvasRuntime {
    pub runtime_state_object_key: String,
    pub share_id: String,
    pub api_base_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiCanvasPureHttpSession {
    pub cookie_header: String,
    pub sapisid: String,
    pub auth_user: String,
}

pub fn share_id_from_share_url(share_url: &str) -> Option<String> {
    let trimmed = share_url.trim();
    if trimmed.is_empty() {
        return None;
    }
    let parsed = Url::parse(trimmed).ok()?;
    let mut segments = parsed.path_segments()?;
    let first = segments.next()?;
    let second = segments.next()?;
    if !first.eq_ignore_ascii_case("share") {
        return None;
    }
    let second = second.trim();
    if second.is_empty() {
        return None;
    }
    Some(second.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiCanvasTextStreamGenerateTemplate {
    pub url: String,
    pub query: Vec<(String, String)>,
    pub form: Vec<(String, String)>,
    pub raw_post_data: String,
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiCanvasStreamGenerateLocator {
    pub response_id: String,
    pub conversation_id: String,
    pub app_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiCanvasConversationListEntry {
    pub conversation_id: String,
    pub title: String,
    pub response_id: Option<String>,
    pub updated_at_secs: i64,
    pub updated_at_nanos: i64,
}

impl GeminiCanvasConversationListEntry {
    pub fn app_path(&self) -> Option<String> {
        let app_conversation = self
            .conversation_id
            .strip_prefix("c_")
            .unwrap_or(self.conversation_id.as_str())
            .trim();
        if app_conversation.is_empty() {
            None
        } else {
            Some(format!("/app/{app_conversation}"))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeminiCanvasMediaOperation {
    Image,
    Music,
    Video,
}

pub fn stream_generate_mode_index(operation: GeminiCanvasMediaOperation) -> i64 {
    match operation {
        GeminiCanvasMediaOperation::Image => GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX,
        GeminiCanvasMediaOperation::Music => GEMINI_CANVAS_STREAM_GENERATE_MUSIC_MODE_INDEX,
        GeminiCanvasMediaOperation::Video => GEMINI_CANVAS_STREAM_GENERATE_VIDEO_MODE_INDEX,
    }
}

pub fn media_operation_selection_preflight_mode_index(mode_index: i64) -> i64 {
    if mode_index == GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX
        || mode_index == GEMINI_CANVAS_STREAM_GENERATE_MUSIC_MODE_INDEX
    {
        // Captured image and music flows both still enter qpEbW through
        // selection mode 11 before StreamGenerate itself switches into their
        // final media mode.
        GEMINI_CANVAS_STREAM_GENERATE_VIDEO_MODE_INDEX
    } else {
        mode_index
    }
}

pub fn media_primary_ku4jyf_mode_index(mode_index: i64) -> i64 {
    if mode_index == GEMINI_CANVAS_STREAM_GENERATE_MUSIC_MODE_INDEX
        || mode_index == GEMINI_CANVAS_STREAM_GENERATE_VIDEO_MODE_INDEX
    {
        GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX
    } else {
        mode_index
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeminiCanvasImageModel {
    Gemini25FlashImagePreview,
    Gemini25FlashImage,
    Gemini31FlashImagePreview,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiCanvasImage {
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiCanvasAudio {
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeminiCanvasMediaAsset {
    pub kind: String,
    pub url: String,
    pub mime_type: String,
    pub download_token: Option<String>,
    pub body_base64: Option<String>,
    pub alt: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_seconds: Option<f64>,
}

pub fn image_stream_generate_template_object_key(runtime_state_object_key: &str) -> String {
    let trimmed = runtime_state_object_key.trim();
    if let Some(prefix) = trimmed.strip_suffix("/storage-state.json") {
        return format!("{prefix}/image-stream-generate-template.json");
    }
    if let Some(prefix) = trimmed.strip_suffix("\\storage-state.json") {
        return format!("{prefix}/image-stream-generate-template.json");
    }
    format!("{trimmed}.image-stream-generate-template.json")
}

pub fn image_edit_stream_generate_template_object_key(runtime_state_object_key: &str) -> String {
    let trimmed = runtime_state_object_key.trim();
    if let Some(prefix) = trimmed.strip_suffix("/storage-state.json") {
        return format!("{prefix}/image-edit-stream-generate-template.json");
    }
    if let Some(prefix) = trimmed.strip_suffix("\\storage-state.json") {
        return format!("{prefix}/image-edit-stream-generate-template.json");
    }
    format!("{trimmed}.image-edit-stream-generate-template.json")
}

pub fn runtime_from_payload(
    payload: &ProviderAccountPayload,
) -> Result<GeminiCanvasRuntime, GatewayError> {
    gemini_canvas_modular::runtime_from_payload(payload)
}

pub fn normalize_video_generations(body: Value) -> Result<CanonicalRelayRequest, GatewayError> {
    normalize_prompt_request(
        body,
        EndpointKind::VideosGenerations,
        GEMINI_CANVAS_VIDEO_PREVIEW_MODEL,
        "Video generation requests require a prompt.",
        "missing_video_prompt",
    )
}

pub fn resolve_image_model(model: &str) -> Result<GeminiCanvasImageModel, GatewayError> {
    gemini_canvas_modular::resolve_image_model(model)
}

pub fn resolve_official_image_model(model: &str) -> Result<&'static str, GatewayError> {
    crate::protocol::gemini::api::resolve_official_image_model(model)
}

pub fn resolve_direct_http_image_model(model: &str) -> Result<&'static str, GatewayError> {
    gemini_canvas_modular::resolve_direct_http_image_model(model)
}

pub fn resolve_music_model(model: &str) -> Result<&'static str, GatewayError> {
    gemini_canvas_modular::resolve_music_model(model)
}

pub fn resolve_official_music_model(model: &str) -> Result<&'static str, GatewayError> {
    crate::protocol::gemini::api::resolve_official_music_model(model)
}

pub fn resolve_video_model(model: &str) -> Result<&'static str, GatewayError> {
    gemini_canvas_modular::resolve_video_model(model)
}

pub fn resolve_official_video_model(model: &str) -> Result<&'static str, GatewayError> {
    crate::protocol::gemini::api::resolve_official_video_model(model)
}

pub fn resolve_text_model(model: &str) -> Result<&str, GatewayError> {
    gemini_canvas_modular::resolve_text_model(model)
}

pub fn prompt_from_request(
    req: &CanonicalRelayRequest,
    missing_message: &str,
    missing_code: &str,
) -> Result<String, GatewayError> {
    gemini_canvas_modular::prompt_from_request(req, missing_message, missing_code)
}

pub(crate) fn latest_nonempty_user_text(req: &CanonicalRelayRequest) -> Option<String> {
    gemini_canvas_modular::latest_nonempty_user_text(req)
}

pub fn prompt_for_text_request(
    req: &CanonicalRelayRequest,
    missing_message: &str,
    missing_code: &str,
) -> Result<String, GatewayError> {
    gemini_canvas_modular::prompt_for_text_request(req, missing_message, missing_code)
}

pub fn requested_output_count(req: &CanonicalRelayRequest) -> usize {
    gemini_canvas_modular::requested_output_count(req)
}

pub fn prefers_url_response(req: &CanonicalRelayRequest) -> Result<bool, GatewayError> {
    gemini_canvas_modular::prefers_url_response(req)
}

pub fn aspect_ratio_from_request(req: &CanonicalRelayRequest) -> String {
    gemini_canvas_modular::aspect_ratio_from_request(req)
}

pub fn locale_from_payload(payload: &ProviderAccountPayload) -> String {
    gemini_canvas_modular::locale_from_payload(payload)
}

pub fn build_text_fetch_url(runtime: &GeminiCanvasRuntime, model: &str) -> String {
    gemini_canvas_modular::build_text_fetch_url(runtime, model)
}

pub fn extract_image_edit_uploads(
    req: &CanonicalRelayRequest,
) -> Result<Vec<GeminiCanvasImageEditUpload>, GatewayError> {
    let uploads = gemini_business::extract_uploads_from_request_body(&req.raw_body)?;
    uploads
        .into_iter()
        .enumerate()
        .map(|(index, upload)| {
            let raw_bytes = base64::engine::general_purpose::STANDARD
                .decode(upload.base64_data.trim())
                .map_err(|error| {
                    GatewayError::bad_request(format!(
                        "Gemini Canvas image edit upload contained invalid base64 bytes: {error}"
                    ))
                    .with_code("invalid_image_edit_upload_base64")
                })?;
            let mime_type = upload.mime_type.trim().to_ascii_lowercase();
            if !mime_type.starts_with("image/") {
                return Err(GatewayError::bad_request(format!(
                    "Gemini Canvas image edit uploads require image mime types, got `{}`.",
                    upload.mime_type
                ))
                .with_code("invalid_image_edit_upload_mime_type"));
            }
            let bytes = normalize_image_edit_upload_to_jpeg(&mime_type, &raw_bytes)?;
            let file_name = if index == 0 {
                "edit-source.jpg".to_string()
            } else {
                format!("edit-source-{}.jpg", index + 1)
            };
            Ok(GeminiCanvasImageEditUpload {
                mime_type: "image/jpeg".to_string(),
                bytes,
                file_name,
                source_mime_type: mime_type,
                source_bytes: raw_bytes,
            })
        })
        .collect()
}

fn normalize_image_edit_upload_to_jpeg(
    mime_type: &str,
    bytes: &[u8],
) -> Result<Vec<u8>, GatewayError> {
    const TARGET_MAX_BYTES: usize = 127_600;
    const TARGET_MIN_SCALE: f32 = 0.18;
    const MAX_LONG_EDGE: u32 = 1400;
    const JPEG_QUALITY: u8 = 82;
    const MAX_JPEG_QUALITY: u8 = 86;
    const MAX_ATTEMPTS: usize = 12;

    let decoded = image::load_from_memory(bytes).map_err(|error| {
        GatewayError::bad_request(format!(
            "Gemini Canvas image edit upload bytes did not decode as an image: {error}"
        ))
        .with_code("invalid_image_edit_upload_image")
    })?;

    let (source_width, source_height) = (decoded.width(), decoded.height());
    let longest_edge = source_width.max(source_height);
    let base_image = if longest_edge > MAX_LONG_EDGE {
        let scale = MAX_LONG_EDGE as f32 / longest_edge as f32;
        let resized_width = ((source_width as f32 * scale).round() as u32).max(1);
        let resized_height = ((source_height as f32 * scale).round() as u32).max(1);
        decoded.resize_exact(
            resized_width,
            resized_height,
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        decoded
    };

    let render_scaled = |scale: f32| -> image::DynamicImage {
        if (scale - 1.0).abs() < f32::EPSILON {
            base_image.clone()
        } else {
            let resized_width = ((base_image.width() as f32 * scale).round() as u32).max(1);
            let resized_height = ((base_image.height() as f32 * scale).round() as u32).max(1);
            base_image.resize_exact(
                resized_width,
                resized_height,
                image::imageops::FilterType::Lanczos3,
            )
        }
    };

    let full_encoded = encode_image_edit_upload_as_jpeg(&base_image, JPEG_QUALITY)?;
    if full_encoded.len() <= TARGET_MAX_BYTES {
        return Ok(full_encoded);
    }

    let mut low_scale = TARGET_MIN_SCALE;
    let mut high_scale = 1.0_f32;
    let mut best_under_limit: Option<(Vec<u8>, f32)> = None;
    let mut best_over_limit: Option<(Vec<u8>, f32)> = Some((full_encoded, 1.0_f32));

    for attempt in 0..MAX_ATTEMPTS {
        let next_scale = if attempt == 0 {
            ((TARGET_MAX_BYTES as f32 / best_over_limit.as_ref().unwrap().0.len() as f32).sqrt()
                * 0.995)
                .clamp(low_scale, high_scale)
        } else {
            ((low_scale + high_scale) / 2.0).clamp(TARGET_MIN_SCALE, 1.0)
        };

        let encoded = encode_image_edit_upload_as_jpeg(&render_scaled(next_scale), JPEG_QUALITY)?;
        let encoded_len = encoded.len();

        if encoded_len <= TARGET_MAX_BYTES {
            let should_replace = match &best_under_limit {
                Some((previous, _)) => encoded_len > previous.len(),
                None => true,
            };
            if should_replace {
                best_under_limit = Some((encoded, next_scale));
            }
            low_scale = next_scale;
        } else {
            let should_replace = match &best_over_limit {
                Some((previous, _)) => encoded_len < previous.len(),
                None => true,
            };
            if should_replace {
                best_over_limit = Some((encoded, next_scale));
            }
            high_scale = next_scale;
        }

        if (high_scale - low_scale) <= 0.01 {
            break;
        }
    }

    if let Some((best_bytes, best_scale)) = best_under_limit.as_mut() {
        let best_image = render_scaled(*best_scale);
        for quality in (JPEG_QUALITY + 1)..=MAX_JPEG_QUALITY {
            let encoded = encode_image_edit_upload_as_jpeg(&best_image, quality)?;
            let encoded_len = encoded.len();
            if encoded_len <= TARGET_MAX_BYTES && encoded_len > best_bytes.len() {
                *best_bytes = encoded;
            }
        }
    }

    best_under_limit
        .map(|(bytes, _)| bytes)
        .or_else(|| best_over_limit.map(|(bytes, _)| bytes))
        .ok_or_else(|| {
            GatewayError::server_error(format!(
            "Gemini Canvas image edit upload could not be normalized from mime type `{mime_type}`."
        ))
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_image_edit_normalize_failed")
        })
}

fn encode_image_edit_upload_as_jpeg(
    image: &image::DynamicImage,
    quality: u8,
) -> Result<Vec<u8>, GatewayError> {
    let rgb = image.to_rgb8();
    let (width, height) = rgb.dimensions();
    let mut encoded = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut encoded, quality);
    encoder
        .encode(&rgb, width, height, ExtendedColorType::Rgb8)
        .map_err(|error| {
            GatewayError::server_error(format!(
                "Gemini Canvas image edit upload JPEG transcode failed: {error}"
            ))
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_image_edit_jpeg_encode_failed")
        })?;
    Ok(encoded)
}

pub fn build_image_request_body(req: &CanonicalRelayRequest, model: &str) -> Value {
    crate::protocol::gemini::api::build_image_request_body(req, model)
}

pub fn build_direct_http_image_request_body(req: &CanonicalRelayRequest, model: &str) -> Value {
    gemini_canvas_modular::build_direct_http_image_request_body(req, model)
}

pub fn build_imagen_predict_request(req: &CanonicalRelayRequest, prompt: &str) -> Value {
    gemini_canvas_modular::build_imagen_predict_request(req, prompt)
}

pub fn direct_http_image_api_base_url(runtime: &GeminiCanvasRuntime) -> String {
    gemini_canvas_modular::direct_http_image_api_base_url(runtime)
}

pub fn build_music_client_content(prompt: &str) -> Value {
    crate::protocol::gemini::api::build_music_client_content(prompt)
}

pub fn build_music_generation_config(req: &CanonicalRelayRequest) -> Value {
    crate::protocol::gemini::api::build_music_generation_config(req)
}

pub fn duration_seconds_from_request(req: &CanonicalRelayRequest) -> Option<f64> {
    req.raw_body
        .get("duration")
        .or_else(|| req.raw_body.get("duration_s"))
        .or_else(|| req.raw_body.get("durationSeconds"))
        .and_then(Value::as_f64)
}

pub fn build_text_request_body(req: &CanonicalRelayRequest, model: &str) -> Value {
    crate::protocol::gemini::api::build_text_request_body(req, model)
}

pub fn build_stream_generate_heavy_request(
    prompt: &str,
    bootstrap: &gemini_web::GeminiWebBootstrap,
    request_uuid: &str,
    mode_index: i64,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    build_stream_generate_heavy_request_with_uploaded_files_seeded(
        prompt,
        bootstrap,
        request_uuid,
        mode_index,
        &[],
        None,
    )
}

pub fn build_stream_generate_heavy_request_with_uploaded_files(
    prompt: &str,
    bootstrap: &gemini_web::GeminiWebBootstrap,
    request_uuid: &str,
    mode_index: i64,
    uploaded_files: &[GeminiCanvasUploadedFileRef],
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    build_stream_generate_heavy_request_with_uploaded_files_seeded(
        prompt,
        bootstrap,
        request_uuid,
        mode_index,
        uploaded_files,
        None,
    )
}

pub fn build_stream_generate_heavy_request_with_uploaded_files_seeded(
    prompt: &str,
    bootstrap: &gemini_web::GeminiWebBootstrap,
    request_uuid: &str,
    mode_index: i64,
    uploaded_files: &[GeminiCanvasUploadedFileRef],
    seed: Option<&GeminiCanvasStreamGenerateSeed>,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    let prompt = prompt.trim();
    if prompt.is_empty() {
        return Err(GatewayError::bad_request(
            "Gemini Canvas StreamGenerate requests require a prompt.",
        )
        .with_code("missing_gemini_canvas_stream_generate_prompt"));
    }
    let language = bootstrap.language.trim();
    let language = if language.is_empty() { "en" } else { language };
    let request_uuid = request_uuid.trim();
    if request_uuid.is_empty() {
        return Err(GatewayError::server_error(
            "Gemini Canvas direct HTTP StreamGenerate request UUID was empty.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_stream_generate_missing_request_uuid"));
    }

    let is_text_mode = mode_index == GEMINI_CANVAS_TEXT_STREAM_GENERATE_TEXT_MODE_INDEX;
    let is_image_mode = mode_index == GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX;
    let is_image_edit_mode = is_image_mode && !uploaded_files.is_empty();
    let seed_opaque_state = seed
        .and_then(|value| value.opaque_state.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let seed_request_hex = seed
        .and_then(|value| value.request_hex.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if !uploaded_files.is_empty() && !is_image_mode {
        return Err(GatewayError::server_error(
            "Gemini Canvas uploaded file refs are only valid for image StreamGenerate mode.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_invalid_uploaded_file_mode"));
    }

    let mut inner_req_list = vec![Value::Null; 80];
    inner_req_list[0] = if uploaded_files.is_empty() {
        json!([prompt, 0, null, null, null, null, 0])
    } else {
        let attachments = uploaded_files
            .iter()
            .map(|file| {
                json!([
                    [file.resource_path, 1, null, file.mime_type],
                    file.file_name,
                    null,
                    null,
                    null,
                    null,
                    null,
                    null,
                    [0]
                ])
            })
            .collect::<Vec<_>>();
        json!([prompt, 0, null, attachments, null, null, 0])
    };
    inner_req_list[1] = json!([language]);
    inner_req_list[2] = json!(["", "", "", null, null, null, null, null, null, ""]);
    inner_req_list[3] = Value::String(
        if is_text_mode {
            GEMINI_CANVAS_TEXT_STREAM_GENERATE_OPAQUE_STATE
        } else if is_image_edit_mode {
            seed_opaque_state.unwrap_or(GEMINI_CANVAS_IMAGE_STREAM_GENERATE_OPAQUE_STATE)
        } else if is_image_mode {
            GEMINI_CANVAS_IMAGE_STREAM_GENERATE_OPAQUE_STATE
        } else {
            GEMINI_CANVAS_MEDIA_STREAM_GENERATE_OPAQUE_STATE
        }
        .to_string(),
    );
    inner_req_list[4] = Value::String(
        seed_request_hex
            .map(ToString::to_string)
            .unwrap_or_else(random_hex_32),
    );
    inner_req_list[6] = if is_text_mode {
        json!([0])
    } else if is_image_edit_mode {
        json!([1])
    } else if is_image_mode {
        json!([0])
    } else {
        json!([1])
    };
    inner_req_list[7] = Value::from(1);
    inner_req_list[10] = Value::from(1);
    inner_req_list[11] = Value::from(0);
    inner_req_list[17] = json!([[0]]);
    inner_req_list[18] = Value::from(0);
    inner_req_list[27] = Value::from(1);
    inner_req_list[30] = json!([4]);
    inner_req_list[41] = if is_text_mode { json!([2]) } else { json!([1]) };
    inner_req_list[49] = Value::from(mode_index);
    inner_req_list[53] = Value::from(0);
    inner_req_list[59] = Value::String(request_uuid.to_string());
    inner_req_list[61] = Value::Array(Vec::new());
    inner_req_list[68] = if is_text_mode {
        Value::from(1)
    } else if is_image_edit_mode {
        Value::from(2)
    } else if is_image_mode {
        Value::from(1)
    } else {
        Value::from(2)
    };
    inner_req_list[79] = Value::from(1);

    let inner_payload = serde_json::to_string(&inner_req_list).map_err(|error| {
        GatewayError::server_error(format!(
            "serialize Gemini Canvas StreamGenerate inner payload: {error}"
        ))
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_stream_generate_serialize_failed")
    })?;
    let f_req = serde_json::to_string(&vec![Value::Null, Value::String(inner_payload)]).map_err(
        |error| {
            GatewayError::server_error(format!(
                "serialize Gemini Canvas StreamGenerate outer payload: {error}"
            ))
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_stream_generate_serialize_failed")
        },
    )?;

    let mut query = vec![
        ("hl".to_string(), language.to_string()),
        ("_reqid".to_string(), current_reqid().to_string()),
        ("rt".to_string(), "c".to_string()),
    ];
    if let Some(build_label) = bootstrap
        .build_label
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query.push(("bl".to_string(), build_label.to_string()));
    }
    if let Some(session_id) = bootstrap
        .session_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query.push(("f.sid".to_string(), session_id.to_string()));
    }

    let mut form = vec![("f.req".to_string(), f_req)];
    if !is_text_mode {
        if let Some(access_token) = bootstrap
            .access_token
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            form.insert(0, ("at".to_string(), access_token.to_string()));
        }
    }

    Ok(gemini_web::GeminiWebRequest { query, form })
}

pub fn build_text_stream_generate_request_from_template(
    storage_state: &Value,
    prompt: &str,
) -> Result<Option<GeminiCanvasTextStreamGenerateTemplate>, GatewayError> {
    build_stream_generate_request_from_template_keys(
        storage_state,
        &["textStreamGenerateTemplate", "streamGenerateTemplate"],
        prompt,
        None,
        None,
    )
}

pub fn build_image_stream_generate_request_from_template(
    storage_state: &Value,
    prompt: &str,
    request_uuid: &str,
) -> Result<Option<GeminiCanvasTextStreamGenerateTemplate>, GatewayError> {
    build_stream_generate_request_from_template_keys(
        storage_state,
        &["imageStreamGenerateTemplate", "mediaStreamGenerateTemplate"],
        prompt,
        Some(request_uuid),
        None,
    )
}

pub fn build_image_edit_stream_generate_request_from_template(
    storage_state: &Value,
    prompt: &str,
    request_uuid: &str,
    uploaded_files: &[GeminiCanvasUploadedFileRef],
) -> Result<Option<GeminiCanvasTextStreamGenerateTemplate>, GatewayError> {
    build_stream_generate_request_from_template_keys(
        storage_state,
        &["imageEditStreamGenerateTemplate", "imageEditStreamTemplate"],
        prompt,
        Some(request_uuid),
        Some(uploaded_files),
    )
}

pub fn harvest_image_edit_stream_generate_seed(
    storage_state: &Value,
) -> Option<GeminiCanvasStreamGenerateSeed> {
    let template = storage_state
        .get("imageEditStreamGenerateTemplate")
        .or_else(|| storage_state.get("imageEditStreamTemplate"))?;
    let template_obj = template.as_object()?;
    let post_data = template_obj
        .get("postData")
        .or_else(|| template_obj.get("rawPostData"))
        .and_then(Value::as_str)?
        .trim();
    if post_data.is_empty() {
        return None;
    }
    let form = url::form_urlencoded::parse(post_data.as_bytes())
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    let f_req = form.iter().find(|(key, _)| key == "f.req")?.1.as_str();
    let outer = serde_json::from_str::<Vec<Value>>(f_req).ok()?;
    let inner_payload = outer.get(1)?.as_str()?;
    let inner = serde_json::from_str::<Vec<Value>>(inner_payload).ok()?;
    Some(GeminiCanvasStreamGenerateSeed {
        opaque_state: inner
            .get(3)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string),
        request_hex: inner
            .get(4)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string),
        request_uuid: inner
            .get(59)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string),
    })
}

fn build_stream_generate_request_from_template_keys(
    storage_state: &Value,
    template_keys: &[&str],
    prompt: &str,
    request_uuid: Option<&str>,
    uploaded_files: Option<&[GeminiCanvasUploadedFileRef]>,
) -> Result<Option<GeminiCanvasTextStreamGenerateTemplate>, GatewayError> {
    let template = template_keys.iter().find_map(|key| storage_state.get(*key));
    let Some(template) = template else {
        return Ok(None);
    };
    let Some(template_obj) = template.as_object() else {
        return Err(GatewayError::server_error(
            "Gemini Canvas stream template must be a JSON object.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_text_stream_template_invalid"));
    };

    let prompt = prompt.trim();
    if prompt.is_empty() {
        return Err(GatewayError::bad_request(
            "Gemini Canvas StreamGenerate requests require a prompt.",
        )
        .with_code("missing_gemini_canvas_stream_generate_prompt"));
    }

    let url = template_obj
        .get("url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            GatewayError::server_error(
                "Gemini Canvas text stream template did not include a non-empty url.",
            )
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_text_stream_template_invalid")
        })?;
    let parsed_url = Url::parse(url).map_err(|error| {
        GatewayError::server_error(format!(
            "parse Gemini Canvas text stream template url: {error}"
        ))
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_text_stream_template_invalid")
    })?;
    let post_data = template_obj
        .get("postData")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            GatewayError::server_error(
                "Gemini Canvas text stream template did not include non-empty postData.",
            )
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_text_stream_template_invalid")
        })?;
    let headers_obj = template_obj
        .get("headers")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            GatewayError::server_error(
                "Gemini Canvas text stream template did not include a headers object.",
            )
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_text_stream_template_invalid")
        })?;

    let headers = headers_obj
        .iter()
        .filter_map(|(name, value)| {
            value
                .as_str()
                .map(|entry| (name.to_ascii_lowercase(), entry.to_string()))
        })
        .collect::<HashMap<_, _>>();
    let query = parsed_url
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    let mut form = url::form_urlencoded::parse(post_data.as_bytes())
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    let Some(f_req_index) = form.iter().position(|(key, _)| key == "f.req") else {
        return Err(GatewayError::server_error(
            "Gemini Canvas text stream template did not include f.req.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_text_stream_template_invalid"));
    };

    let mut outer = serde_json::from_str::<Vec<Value>>(&form[f_req_index].1).map_err(|error| {
        GatewayError::server_error(format!(
            "parse Gemini Canvas text stream template f.req outer payload: {error}"
        ))
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_text_stream_template_invalid")
    })?;
    let Some(inner_payload) = outer.get(1).and_then(Value::as_str) else {
        return Err(GatewayError::server_error(
            "Gemini Canvas text stream template outer payload did not contain the inner payload string.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_text_stream_template_invalid"));
    };
    let mut inner = serde_json::from_str::<Vec<Value>>(inner_payload).map_err(|error| {
        GatewayError::server_error(format!(
            "parse Gemini Canvas text stream template inner payload: {error}"
        ))
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_text_stream_template_invalid")
    })?;
    let Some(prompt_parts) = inner.get_mut(0).and_then(Value::as_array_mut) else {
        return Err(GatewayError::server_error(
            "Gemini Canvas text stream template inner payload did not contain the prompt array.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_text_stream_template_invalid"));
    };
    if prompt_parts.is_empty() {
        return Err(GatewayError::server_error(
            "Gemini Canvas text stream template prompt array was empty.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_text_stream_template_invalid"));
    }
    prompt_parts[0] = Value::String(prompt.to_string());
    if let Some(uploaded_files) = uploaded_files {
        while prompt_parts.len() <= 6 {
            prompt_parts.push(Value::Null);
        }
        let attachments = uploaded_files
            .iter()
            .map(|file| {
                json!([
                    [file.resource_path, 1, null, file.mime_type],
                    file.file_name,
                    null,
                    null,
                    null,
                    null,
                    null,
                    null,
                    [0]
                ])
            })
            .collect::<Vec<_>>();
        prompt_parts[3] = Value::Array(attachments);
        prompt_parts[6] = Value::from(0);
        if inner.len() > 6 {
            inner[6] = json!([1]);
        }
        if inner.len() > 68 {
            inner[68] = Value::from(2);
        }
    }
    if let Some(request_uuid) = request_uuid
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if inner.len() > 59 {
            inner[59] = Value::String(request_uuid.to_string());
        }
    }
    outer[1] = Value::String(serde_json::to_string(&inner).map_err(|error| {
        GatewayError::server_error(format!(
            "serialize Gemini Canvas text stream template inner payload: {error}"
        ))
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_text_stream_template_invalid")
    })?);
    form[f_req_index].1 = serde_json::to_string(&outer).map_err(|error| {
        GatewayError::server_error(format!(
            "serialize Gemini Canvas text stream template outer payload: {error}"
        ))
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_text_stream_template_invalid")
    })?;

    let mut headers = headers;
    if let Some(request_uuid) = request_uuid
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Some(existing_header) = headers
            .get("x-goog-ext-525005358-jspb")
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let updated_header = serde_json::from_str::<Vec<Value>>(existing_header)
                .ok()
                .and_then(|mut parsed| {
                    if parsed.is_empty() {
                        return None;
                    }
                    parsed[0] = Value::String(request_uuid.to_string());
                    serde_json::to_string(&parsed).ok()
                })
                .unwrap_or_else(|| format!("[\"{request_uuid}\",1]"));
            headers.insert("x-goog-ext-525005358-jspb".to_string(), updated_header);
        }
    }

    let mut request_url = parsed_url.clone();
    request_url.set_query(None);
    request_url.set_fragment(None);
    let raw_post_data = serialize_stream_generate_form_pairs(&form);

    Ok(Some(GeminiCanvasTextStreamGenerateTemplate {
        url: request_url.to_string(),
        query,
        form,
        raw_post_data,
        headers,
    }))
}

pub fn harvest_text_batchexecute_header_id(storage_state: &Value) -> Option<String> {
    let template = storage_state
        .get("textStreamGenerateTemplate")
        .or_else(|| storage_state.get("streamGenerateTemplate"))?;
    let headers = template.get("headers")?.as_object()?;
    let header = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("x-goog-ext-525001261-jspb"))
        .and_then(|(_, value)| value.as_str())?
        .trim();
    if header.is_empty() {
        return None;
    }
    let parsed = serde_json::from_str::<Vec<Value>>(header).ok()?;
    parsed
        .get(16)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub fn harvest_batchexecute_header_id_for_mode(
    storage_state: &Value,
    mode_index: i64,
) -> Option<String> {
    let template = if mode_index == GEMINI_CANVAS_TEXT_STREAM_GENERATE_TEXT_MODE_INDEX {
        storage_state
            .get("textStreamGenerateTemplate")
            .or_else(|| storage_state.get("streamGenerateTemplate"))?
    } else {
        storage_state
            .get("imageStreamGenerateTemplate")
            .or_else(|| storage_state.get("mediaStreamGenerateTemplate"))
            .or_else(|| storage_state.get("textStreamGenerateTemplate"))
            .or_else(|| storage_state.get("streamGenerateTemplate"))?
    };
    let headers = template.get("headers")?.as_object()?;
    let header = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("x-goog-ext-525001261-jspb"))
        .and_then(|(_, value)| value.as_str())?
        .trim();
    if header.is_empty() {
        return None;
    }
    let parsed = serde_json::from_str::<Vec<Value>>(header).ok()?;
    parsed
        .get(16)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub fn harvest_image_edit_template_locale(storage_state: &Value) -> Option<String> {
    for key in [
        "imageEditStreamGenerateTemplate",
        "imageEditStreamTemplate",
        "imageStreamGenerateTemplate",
        "mediaStreamGenerateTemplate",
        "textStreamGenerateTemplate",
        "streamGenerateTemplate",
    ] {
        let Some(template) = storage_state.get(key).and_then(Value::as_object) else {
            continue;
        };
        if let Some(locale) = template
            .get("url")
            .and_then(Value::as_str)
            .and_then(|value| Url::parse(value).ok())
            .and_then(|url| {
                url.query_pairs()
                    .find(|(name, _)| name == "hl")
                    .map(|(_, value)| value.trim().to_string())
            })
            .filter(|value| !value.is_empty())
        {
            return Some(locale);
        }
        if let Some(locale) = template
            .get("headers")
            .and_then(Value::as_object)
            .and_then(|headers| {
                headers
                    .iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case("accept-language"))
                    .and_then(|(_, value)| value.as_str())
            })
            .and_then(|value| value.split(',').next())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
        {
            return Some(locale);
        }
    }
    None
}

pub fn build_stream_generate_model_header_from_storage_state(
    storage_state: &Value,
    mode_index: i64,
    prefer_image_edit_template: bool,
) -> Option<String> {
    let (template, image_specific_template) =
        if mode_index == GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX {
            if prefer_image_edit_template {
                if let Some(template) = storage_state.get("imageEditStreamGenerateTemplate") {
                    (template, true)
                } else if let Some(template) = storage_state.get("imageEditStreamTemplate") {
                    (template, true)
                } else if let Some(template) = storage_state.get("imageStreamGenerateTemplate") {
                    (template, true)
                } else if let Some(template) = storage_state.get("mediaStreamGenerateTemplate") {
                    (template, true)
                } else if let Some(template) = storage_state.get("textStreamGenerateTemplate") {
                    (template, false)
                } else {
                    (storage_state.get("streamGenerateTemplate")?, false)
                }
            } else if let Some(template) = storage_state.get("imageStreamGenerateTemplate") {
                (template, true)
            } else if let Some(template) = storage_state.get("mediaStreamGenerateTemplate") {
                (template, true)
            } else if let Some(template) = storage_state.get("textStreamGenerateTemplate") {
                (template, false)
            } else {
                (storage_state.get("streamGenerateTemplate")?, false)
            }
        } else {
            (
                storage_state
                    .get("textStreamGenerateTemplate")
                    .or_else(|| storage_state.get("streamGenerateTemplate"))?,
                false,
            )
        };
    let headers = template.get("headers")?.as_object()?;
    let header = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("x-goog-ext-525001261-jspb"))
        .and_then(|(_, value)| value.as_str())?
        .trim();
    if header.is_empty() {
        return None;
    }

    let mut parsed = serde_json::from_str::<Vec<Value>>(header).ok()?;
    if parsed.len() <= 11 {
        return None;
    }

    let is_text_mode = mode_index == GEMINI_CANVAS_TEXT_STREAM_GENERATE_TEXT_MODE_INDEX;
    if is_text_mode {
        parsed[4] = Value::String(GEMINI_CANVAS_TEXT_LAST_SELECTED_MODE_ID.to_string());
    } else {
        let current = parsed.get(4).and_then(Value::as_str).map(str::trim);
        if !image_specific_template || current.map(|value| value.is_empty()).unwrap_or(true) {
            parsed[4] = Value::String(GEMINI_CANVAS_TEXT_SELECTED_MODEL_HEADER_ID.to_string());
        }
    }
    parsed[11] = Value::from(if is_text_mode { 1 } else { 2 });
    serde_json::to_string(&parsed).ok()
}

pub fn refresh_stream_generate_template_with_bootstrap(
    template: &GeminiCanvasTextStreamGenerateTemplate,
    bootstrap: &gemini_web::GeminiWebBootstrap,
    include_access_token: bool,
) -> Result<GeminiCanvasTextStreamGenerateTemplate, GatewayError> {
    let mut parsed_url = Url::parse(&template.url).map_err(|error| {
        GatewayError::server_error(format!(
            "parse Gemini Canvas stream template url for bootstrap refresh: {error}"
        ))
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_text_stream_template_invalid")
    })?;
    parsed_url.set_query(None);
    parsed_url.set_fragment(None);

    let mut query = template.query.clone();
    upsert_stream_generate_query_param(
        &mut query,
        "bl",
        bootstrap
            .build_label
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty()),
    );
    upsert_stream_generate_query_param(
        &mut query,
        "f.sid",
        bootstrap
            .session_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty()),
    );
    upsert_stream_generate_query_param(&mut query, "hl", Some(bootstrap.language.as_str()));

    let mut form = template.form.clone();
    if include_access_token {
        upsert_stream_generate_form_param(
            &mut form,
            "at",
            bootstrap
                .access_token
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        );
    }
    let raw_post_data = serialize_stream_generate_form_pairs(&form);

    Ok(GeminiCanvasTextStreamGenerateTemplate {
        url: parsed_url.to_string(),
        query,
        form,
        raw_post_data,
        headers: template.headers.clone(),
    })
}

pub fn refresh_stream_generate_template_access_token(
    template: &GeminiCanvasTextStreamGenerateTemplate,
    access_token: &str,
) -> GeminiCanvasTextStreamGenerateTemplate {
    let mut form = template.form.clone();
    upsert_stream_generate_form_param(&mut form, "at", Some(access_token.trim()));
    let raw_post_data = serialize_stream_generate_form_pairs(&form);
    GeminiCanvasTextStreamGenerateTemplate {
        url: template.url.clone(),
        query: template.query.clone(),
        form,
        raw_post_data,
        headers: template.headers.clone(),
    }
}

pub fn refresh_stream_generate_template_model_header_id(
    template: &mut GeminiCanvasTextStreamGenerateTemplate,
    header_id: &str,
) -> bool {
    let header_id = header_id.trim();
    if header_id.is_empty() {
        return false;
    }
    let Some(existing) = template
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("x-goog-ext-525001261-jspb"))
        .map(|(_, value)| value.clone())
    else {
        return false;
    };
    let Ok(mut parsed) = serde_json::from_str::<Vec<Value>>(&existing) else {
        return false;
    };
    if parsed.len() <= 16 {
        return false;
    }
    parsed[16] = Value::String(header_id.to_string());
    let Ok(updated) = serde_json::to_string(&parsed) else {
        return false;
    };
    if let Some((_, value)) = template
        .headers
        .iter_mut()
        .find(|(name, _)| name.eq_ignore_ascii_case("x-goog-ext-525001261-jspb"))
    {
        *value = updated;
        true
    } else {
        false
    }
}

pub fn refresh_stream_generate_template_request_hex(
    template: &mut GeminiCanvasTextStreamGenerateTemplate,
    request_hex: &str,
) -> bool {
    let request_hex = request_hex.trim();
    if request_hex.is_empty() {
        return false;
    }
    if request_hex.len() != 32 || !request_hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return false;
    }
    let Some(f_req_index) = template.form.iter().position(|(key, _)| key == "f.req") else {
        return false;
    };
    let Ok(mut outer) = serde_json::from_str::<Vec<Value>>(&template.form[f_req_index].1) else {
        return false;
    };
    let Some(inner_payload) = outer.get(1).and_then(Value::as_str) else {
        return false;
    };
    let Ok(mut inner) = serde_json::from_str::<Vec<Value>>(inner_payload) else {
        return false;
    };
    if inner.len() <= 4 {
        return false;
    }
    inner[4] = Value::String(request_hex.to_string());
    let Ok(inner_json) = serde_json::to_string(&inner) else {
        return false;
    };
    outer[1] = Value::String(inner_json);
    let Ok(outer_json) = serde_json::to_string(&outer) else {
        return false;
    };
    template.form[f_req_index].1 = outer_json;
    template.raw_post_data = serialize_stream_generate_form_pairs(&template.form);
    true
}

fn serialize_stream_generate_form_pairs(form: &[(String, String)]) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in form {
        serializer.append_pair(key, value);
    }
    serializer.finish()
}

fn upsert_stream_generate_query_param(
    query: &mut Vec<(String, String)>,
    key: &str,
    value: Option<&str>,
) {
    if let Some(existing) = query.iter_mut().find(|(name, _)| name == key) {
        if let Some(value) = value {
            existing.1 = value.to_string();
        }
    } else if let Some(value) = value {
        query.push((key.to_string(), value.to_string()));
    }
}

fn upsert_stream_generate_form_param(
    form: &mut Vec<(String, String)>,
    key: &str,
    value: Option<&str>,
) {
    if let Some(value) = value {
        if let Some(existing) = form.iter_mut().find(|(name, _)| name == key) {
            existing.1 = value.to_string();
            return;
        }
        form.push((key.to_string(), value.to_string()));
        return;
    }
    form.retain(|(name, _)| name != key);
}

pub fn build_text_batchexecute_model_header_variant(
    model_id: Option<&str>,
    header_id: Option<&str>,
    include_mode_flag: bool,
    force_empty_model_id: bool,
) -> String {
    let mut header = vec![
        Value::from(1),
        Value::Null,
        Value::Null,
        Value::Null,
        Value::Null,
        Value::Null,
        Value::Null,
        Value::Null,
        json!([4]),
        Value::Null,
        Value::Null,
        Value::Null,
        Value::Null,
        Value::Null,
        if include_mode_flag {
            Value::from(1)
        } else {
            Value::Null
        },
        Value::Null,
        Value::String(
            header_id
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(GEMINI_CANVAS_TEXT_PREFLIGHT_HEADER_ID_DEFAULT)
                .to_string(),
        ),
    ];
    if force_empty_model_id {
        header[4] = Value::String(String::new());
    } else if let Some(model_id) = model_id.map(str::trim).filter(|value| !value.is_empty()) {
        header[4] = Value::String(model_id.to_string());
    }
    serde_json::to_string(&header).unwrap_or_else(|_| {
        if model_id.is_some() {
            GEMINI_CANVAS_TEXT_BOOTSTRAP_MODEL_HEADER.to_string()
        } else {
            GEMINI_CANVAS_TEXT_MODE_SELECTION_MODEL_HEADER.to_string()
        }
    })
}

pub fn build_text_batchexecute_model_header(
    model_id: Option<&str>,
    header_id: Option<&str>,
) -> String {
    build_text_batchexecute_model_header_variant(model_id, header_id, true, false)
}

fn normalize_source_path(source_path: &str) -> String {
    let trimmed = source_path.trim();
    if trimmed.is_empty() {
        "/app".to_string()
    } else if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

pub fn build_text_batchexecute_request(
    rpcid: &str,
    payload: Value,
    bootstrap: &gemini_web::GeminiWebBootstrap,
    source_path: &str,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    let language = bootstrap.language.trim();
    let language = if language.is_empty() { "en" } else { language };
    let inner_payload = serde_json::to_string(&payload).map_err(|error| {
        GatewayError::server_error(format!(
            "serialize Gemini Canvas batchexecute payload: {error}"
        ))
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_batchexecute_serialize_failed")
    })?;
    let rpc = Value::Array(vec![
        Value::String(rpcid.to_string()),
        Value::String(inner_payload),
        Value::Null,
        Value::String("generic".to_string()),
    ]);
    let f_req = serde_json::to_string(&vec![Value::Array(vec![rpc])]).map_err(|error| {
        GatewayError::server_error(format!(
            "serialize Gemini Canvas batchexecute wrapper: {error}"
        ))
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_batchexecute_serialize_failed")
    })?;

    let mut query = vec![
        ("rpcids".to_string(), rpcid.to_string()),
        (
            "source-path".to_string(),
            normalize_source_path(source_path),
        ),
        ("hl".to_string(), language.to_string()),
        ("_reqid".to_string(), current_reqid().to_string()),
        ("rt".to_string(), "c".to_string()),
    ];
    if let Some(build_label) = bootstrap
        .build_label
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query.push(("bl".to_string(), build_label.to_string()));
    }
    if let Some(session_id) = bootstrap
        .session_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query.push(("f.sid".to_string(), session_id.to_string()));
    }

    let mut form = vec![("f.req".to_string(), f_req)];
    if let Some(access_token) = bootstrap
        .access_token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        form.push(("at".to_string(), access_token.to_string()));
    }

    Ok(gemini_web::GeminiWebRequest { query, form })
}

pub fn build_text_state_variant_preflight_request(
    bootstrap: &gemini_web::GeminiWebBootstrap,
    source_path: &str,
    state_len: usize,
    tail_index: usize,
    tail_value: Value,
    marker: &str,
    rpcid: &str,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    let mut state = vec![Value::Null; state_len];
    if tail_index >= state_len {
        return Err(GatewayError::server_error(format!(
            "Gemini Canvas state preflight tail index {tail_index} was out of bounds for length {state_len}."
        ))
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_state_preflight_invalid_shape"));
    }
    state[tail_index] = tail_value;
    let payload = vec![
        Value::Array(state),
        Value::Array(vec![Value::Array(vec![Value::String(marker.to_string())])]),
    ];
    build_text_batchexecute_request(rpcid, Value::Array(payload), bootstrap, source_path)
}

pub fn build_mode_selection_preflight_request(
    bootstrap: &gemini_web::GeminiWebBootstrap,
    source_path: &str,
    selected_id: &str,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    build_text_state_variant_preflight_request(
        bootstrap,
        source_path,
        100,
        99,
        Value::String(selected_id.to_string()),
        "last_selected_mode_id_on_web",
        GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID,
    )
}

pub fn build_text_mode_selection_preflight_request(
    bootstrap: &gemini_web::GeminiWebBootstrap,
    source_path: &str,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    build_mode_selection_preflight_request(
        bootstrap,
        source_path,
        GEMINI_CANVAS_TEXT_LAST_SELECTED_MODE_ID,
    )
}

pub fn build_media_operation_selection_preflight_request(
    bootstrap: &gemini_web::GeminiWebBootstrap,
    source_path: &str,
    mode_index: i64,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    let mode_index = media_operation_selection_preflight_mode_index(mode_index);
    build_text_batchexecute_request(
        GEMINI_CANVAS_MEDIA_OPERATION_SELECTION_RPCID,
        json!([[[1, mode_index], [2, mode_index], [6, mode_index]]]),
        bootstrap,
        source_path,
    )
}

pub fn build_text_bootstrap_preflight_request(
    bootstrap: &gemini_web::GeminiWebBootstrap,
    source_path: &str,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    build_text_batchexecute_request(
        GEMINI_CANVAS_TEXT_BOOTSTRAP_RPCID,
        Value::Array(Vec::new()),
        bootstrap,
        source_path,
    )
}

pub fn build_conversation_list_probe_request(
    bootstrap: &gemini_web::GeminiWebBootstrap,
    source_path: &str,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    build_text_batchexecute_request(
        "MaZiqc",
        json!([13, null, [1, null, 1]]),
        bootstrap,
        source_path,
    )
}

pub fn build_conversation_list_full_request(
    bootstrap: &gemini_web::GeminiWebBootstrap,
    source_path: &str,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    build_text_batchexecute_request(
        "MaZiqc",
        json!([13, null, [0, null, 1]]),
        bootstrap,
        source_path,
    )
}

pub fn build_video_job_poll_request(
    bootstrap: &gemini_web::GeminiWebBootstrap,
    source_path: &str,
    job_id: &str,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    build_text_batchexecute_request(
        "kwDCne",
        Value::Array(vec![Value::String(job_id.trim().to_string())]),
        bootstrap,
        source_path,
    )
}

pub fn build_video_completion_followup_request(
    bootstrap: &gemini_web::GeminiWebBootstrap,
    source_path: &str,
    conversation_id: &str,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    build_text_batchexecute_request(
        "hNvQHb",
        json!([conversation_id.trim(), 10, null, 1, [1], [4], null, 1]),
        bootstrap,
        source_path,
    )
}

pub fn build_video_metadata_followup_request(
    bootstrap: &gemini_web::GeminiWebBootstrap,
    source_path: &str,
    conversation_id: &str,
    response_id: &str,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    let conversation_id = conversation_id.trim();
    let response_id = response_id.trim();
    if conversation_id.is_empty() || response_id.is_empty() {
        return Err(GatewayError::server_error(
            "Gemini Canvas video metadata follow-up requires non-empty conversation_id and response_id.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_video_metadata_followup_missing_ids"));
    }

    build_text_batchexecute_request(
        "MUAZcd",
        json!([
            null,
            [["unread_metadata"]],
            [
                conversation_id,
                null,
                null,
                null,
                null,
                null,
                [[conversation_id, response_id], 0]
            ]
        ]),
        bootstrap,
        source_path,
    )
}

pub fn build_text_state_preflight_request(
    bootstrap: &gemini_web::GeminiWebBootstrap,
    source_path: &str,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    build_text_batchexecute_request(
        GEMINI_CANVAS_TEXT_STATE_PREFLIGHT_RPCID,
        json!([[null, null, null, null, true]]),
        bootstrap,
        source_path,
    )
}

pub fn build_image_state_keys_preflight_request(
    bootstrap: &gemini_web::GeminiWebBootstrap,
    source_path: &str,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    build_text_batchexecute_request(
        GEMINI_CANVAS_TEXT_STATE_PREFLIGHT_RPCID,
        json!([[GEMINI_CANVAS_IMAGE_STATE_KEYS_PREFLIGHT_KEYS]]),
        bootstrap,
        source_path,
    )
}

pub fn build_tts_trigger_request(
    response_id: &str,
    bootstrap: &gemini_web::GeminiWebBootstrap,
    source_path: &str,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    let response_id = response_id.trim();
    if response_id.is_empty() {
        return Err(GatewayError::server_error(
            "Gemini Canvas TTS trigger requires a non-empty StreamGenerate response id.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_tts_missing_response_id"));
    }
    build_text_batchexecute_request(
        GEMINI_CANVAS_TTS_TRIGGER_RPCID,
        Value::Array(vec![Value::String(response_id.to_string())]),
        bootstrap,
        source_path,
    )
}

pub fn build_music_trigger_request(
    response_id: &str,
    bootstrap: &gemini_web::GeminiWebBootstrap,
    source_path: &str,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    build_tts_trigger_request(response_id, bootstrap, source_path)
}

pub fn infer_tts_export_locale(text: &str, fallback_locale: &str) -> String {
    let fallback = fallback_locale.trim();
    let fallback = if fallback.is_empty() {
        "en-US"
    } else {
        fallback
    };
    let mut segments = fallback.split(['-', '_']);
    let fallback_language = segments
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("en");
    let region = segments
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("US")
        .to_ascii_uppercase();
    let detected_language = if text.chars().any(is_hangul) {
        "ko"
    } else if text.chars().any(is_hiragana_or_katakana) {
        "ja"
    } else if text.chars().any(is_cjk_unified_ideograph) {
        "zh"
    } else if text.chars().any(is_cyrillic) {
        "ru"
    } else if text.chars().any(|ch| ch.is_ascii_alphabetic()) {
        "en"
    } else {
        fallback_language
    };
    format!("{detected_language}-{region}")
}

pub fn build_tts_audio_export_request(
    text: &str,
    locale: &str,
    bootstrap: &gemini_web::GeminiWebBootstrap,
    source_path: &str,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(GatewayError::server_error(
            "Gemini Canvas TTS export requires a non-empty response text payload.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_tts_missing_export_text"));
    }
    let locale = locale.trim();
    if locale.is_empty() {
        return Err(GatewayError::server_error(
            "Gemini Canvas TTS export requires a non-empty locale tag.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_tts_missing_export_locale"));
    }
    build_text_batchexecute_request(
        GEMINI_CANVAS_TTS_EXPORT_RPCID,
        Value::Array(vec![
            Value::Null,
            Value::String(text.to_string()),
            Value::String(locale.to_string()),
            Value::Null,
            Value::from(2),
        ]),
        bootstrap,
        source_path,
    )
}

pub fn build_text_stream_generate_heavy_request(
    req: &CanonicalRelayRequest,
    bootstrap: &gemini_web::GeminiWebBootstrap,
    request_uuid: &str,
) -> Result<gemini_web::GeminiWebRequest, GatewayError> {
    let prompt = prompt_for_text_request(
        req,
        "Gemini Canvas requests require a prompt.",
        "missing_gemini_canvas_text_prompt",
    )?;
    build_stream_generate_heavy_request(
        &prompt,
        bootstrap,
        request_uuid,
        GEMINI_CANVAS_TEXT_STREAM_GENERATE_TEXT_MODE_INDEX,
    )
}

pub fn new_text_stream_generate_request_uuid() -> String {
    new_stream_generate_request_uuid()
}

pub fn new_batchexecute_header_id() -> String {
    uuid::Uuid::new_v4().to_string().to_ascii_uppercase()
}

pub fn new_stream_generate_request_uuid() -> String {
    new_batchexecute_header_id()
}

fn random_hex_32() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

fn current_reqid() -> u64 {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    10_000 + (millis % 89_999)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeminiCanvasPureHttpMode {
    Disabled,
    Preferred,
    Required,
}

pub fn pure_http_mode(payload: &ProviderAccountPayload) -> GeminiCanvasPureHttpMode {
    payload
        .extra_body
        .as_ref()
        .and_then(|extra| read_optional_hash_string(extra, &["pureHttpMode", "pure_http_mode"]))
        .map(|mode| {
            let normalized = mode.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "required" | "force" | "forced" | "strict" => GeminiCanvasPureHttpMode::Required,
                "0" | "false" | "off" | "disabled" | "none" => GeminiCanvasPureHttpMode::Disabled,
                "1" | "true" | "on" | "enabled" | "preferred" | "yes" => {
                    GeminiCanvasPureHttpMode::Preferred
                }
                _ => GeminiCanvasPureHttpMode::Preferred,
            }
        })
        .unwrap_or_else(|| {
            if std::env::var("GEMINI_CANVAS_PURE_HTTP_ENABLED")
                .ok()
                .map(|value| {
                    matches!(
                        value.trim().to_ascii_lowercase().as_str(),
                        "1" | "true" | "yes" | "on"
                    )
                })
                .unwrap_or(true)
            {
                GeminiCanvasPureHttpMode::Preferred
            } else {
                GeminiCanvasPureHttpMode::Disabled
            }
        })
}

pub fn pure_http_enabled(payload: &ProviderAccountPayload) -> bool {
    pure_http_mode(payload) != GeminiCanvasPureHttpMode::Disabled
}

pub fn pure_http_required(payload: &ProviderAccountPayload) -> bool {
    pure_http_mode(payload) == GeminiCanvasPureHttpMode::Required
}

pub fn browser_runtime_state_object_key(payload: &ProviderAccountPayload) -> Option<String> {
    payload
        .extra_body
        .as_ref()
        .and_then(|extra| {
            read_optional_hash_string(
                extra,
                &[
                    "browserRuntimeStateObjectKey",
                    "browserProfileRuntimeStateObjectKey",
                    "browser_runtime_state_object_key",
                    "browser_profile_runtime_state_object_key",
                ],
            )
        })
        .or_else(|| {
            payload
                .runtime_state_object_key
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
        })
}

pub fn browser_cdp_url(payload: &ProviderAccountPayload) -> Option<String> {
    payload.extra_body.as_ref().and_then(|extra| {
        read_optional_hash_string(
            extra,
            &["browserCdpUrl", "browser_cdp_url", "browserDebuggerAddress"],
        )
    })
}

pub fn browser_cookie_header(payload: &ProviderAccountPayload) -> Option<String> {
    payload
        .extra_body
        .as_ref()
        .and_then(|extra| read_optional_hash_string(extra, &["cookieHeader", "cookie_header"]))
        .or_else(|| {
            payload.headers.iter().find_map(|(key, value)| {
                if key.eq_ignore_ascii_case("cookie") {
                    let trimmed = value.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_string())
                    }
                } else {
                    None
                }
            })
        })
}

pub fn image_json_fallback_enabled(payload: &ProviderAccountPayload) -> bool {
    payload
        .extra_body
        .as_ref()
        .and_then(|extra| {
            read_optional_hash_string(
                extra,
                &[
                    "imageJsonFallbackEnabled",
                    "image_json_fallback_enabled",
                    "imageJsonFallback",
                    "image_json_fallback",
                ],
            )
        })
        .map(|mode| {
            matches!(
                mode.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on" | "enabled"
            )
        })
        .unwrap_or_else(|| {
            std::env::var("GEMINI_CANVAS_IMAGE_JSON_FALLBACK_ENABLED")
                .ok()
                .map(|value| {
                    matches!(
                        value.trim().to_ascii_lowercase().as_str(),
                        "1" | "true" | "yes" | "on"
                    )
                })
                .unwrap_or(false)
        })
}

pub fn direct_http_auth_user(payload: &ProviderAccountPayload) -> String {
    payload
        .extra_body
        .as_ref()
        .and_then(|extra| {
            read_optional_hash_string(extra, &["googleAuthUser", "authUser", "auth_user"])
        })
        .unwrap_or_else(|| "0".to_string())
}

pub fn direct_http_google_api_key(
    payload: &ProviderAccountPayload,
    storage_state: &Value,
) -> Option<String> {
    payload
        .extra_body
        .as_ref()
        .and_then(|extra_body| {
            read_optional_hash_string(
                extra_body,
                &["googleApiKey", "google_api_key", "apiKey", "api_key"],
            )
        })
        .or_else(|| {
            extract_google_api_keys_from_value(storage_state)
                .into_iter()
                .next()
        })
        .or_else(|| {
            payload.extra_body.as_ref().and_then(|extra_body| {
                read_optional_hash_string_array_values(extra_body)
                    .into_iter()
                    .next()
            })
        })
        .or_else(|| {
            let api_key = payload.api_key.trim();
            if api_key.is_empty() {
                None
            } else {
                Some(api_key.to_string())
            }
        })
}

pub fn direct_http_google_api_keys(
    payload: &ProviderAccountPayload,
    storage_state: &Value,
) -> Vec<String> {
    let mut keys = Vec::new();
    let has_explicit_extra_primary = payload.extra_body.as_ref().is_some_and(|extra_body| {
        read_optional_hash_string(
            extra_body,
            &["googleApiKey", "google_api_key", "apiKey", "api_key"],
        )
        .is_some()
    });
    if !has_explicit_extra_primary && !payload.api_key.trim().is_empty() {
        extend_unique_strings(&mut keys, vec![payload.api_key.trim().to_string()]);
    }
    if let Some(extra_body) = payload.extra_body.as_ref() {
        extend_unique_strings(&mut keys, extract_google_api_keys_from_hash_map(extra_body));
    }
    extend_unique_strings(&mut keys, extract_google_api_keys_from_value(storage_state));
    keys
}

pub fn extract_signaler_account_id_from_page_blob(blob: &str) -> Option<String> {
    static SIGNALER_ACCOUNT_ID_REGEX: OnceLock<Regex> = OnceLock::new();
    SIGNALER_ACCOUNT_ID_REGEX
        .get_or_init(|| {
            Regex::new(r#""S06Grb":"([^"]+)""#)
                .expect("gemini canvas signaler account id regex must compile")
        })
        .captures(blob)
        .and_then(|captures| captures.get(1))
        .map(|capture| capture.as_str().trim())
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub fn extract_signaler_account_id_from_runtime_state(storage_state: &Value) -> Option<String> {
    let extract_from_value = |value: &Value| -> Option<String> {
        for key in ["signalerAccountId", "googleAccountId", "accountId"] {
            if let Some(found) = value
                .get(key)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|candidate| !candidate.is_empty())
            {
                return Some(found.to_string());
            }
        }
        None
    };

    if let Some(found) = extract_from_value(storage_state) {
        return Some(found);
    }

    for wrapper_key in [
        "storageState",
        "runtimeState",
        "browserState",
        "metadata",
        "state",
    ] {
        if let Some(found) = storage_state.get(wrapper_key).and_then(extract_from_value) {
            return Some(found);
        }
    }

    None
}

pub fn build_image_edit_signaler_choose_server_body(account_id: &str) -> String {
    json!([[
        null,
        null,
        null,
        [9, 5],
        null,
        [["assistant-bard"], [1], [[["async_bard"], [account_id]]]],
        null,
        null,
        0,
        0
    ]])
    .to_string()
}

pub fn build_image_edit_signaler_open_channel_body(account_id: &str) -> String {
    let assistant_entry = |channel: &str, include_account: bool| {
        let channel_payload = if include_account {
            json!([[[channel], [account_id]]])
        } else {
            json!([[[channel]]])
        };
        json!([
            null,
            null,
            null,
            [9, 5],
            null,
            [["assistant-bard"], [1], channel_payload],
            null,
            null,
            1
        ])
    };
    let sync_entry = json!([
        null,
        null,
        null,
        [9, 5],
        null,
        [["assistant-bard"], [null, 1], [[["bard-client-sync"]]]],
        null,
        null,
        1
    ]);
    let request_entries = vec![
        (
            "req0___data__",
            json!([[[1, assistant_entry("async_bard", true), null, 3]]]),
        ),
        (
            "req1___data__",
            json!([[[2, assistant_entry("beyond_agency_bard", true), null, 3]]]),
        ),
        (
            "req2___data__",
            json!([[[
                3,
                assistant_entry("mini_app_generation_bard", true),
                null,
                3
            ]]]),
        ),
        ("req3___data__", json!([[[4, sync_entry, null, 3]]])),
    ];
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    serializer.append_pair("count", "4");
    serializer.append_pair("ofs", "0");
    for (key, value) in request_entries {
        serializer.append_pair(&key, &value.to_string());
    }
    serializer.finish()
}

pub fn build_image_edit_signaler_refresh_creds_body(refresh_token: &str) -> String {
    json!([refresh_token]).to_string()
}

pub fn parse_signaler_choose_server_response(body: &str) -> Result<String, GatewayError> {
    let parsed: Vec<Value> = serde_json::from_str(body.trim()).map_err(|error| {
        GatewayError::server_error(format!(
            "parse Gemini Canvas signaler chooseServer response: {error}"
        ))
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_signaler_choose_server_invalid")
    })?;
    parsed
        .first()
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| {
            GatewayError::server_error(
                "Gemini Canvas signaler chooseServer response did not expose a gsessionid."
                    .to_string(),
            )
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_signaler_choose_server_missing_gsessionid")
        })
}

pub fn parse_signaler_open_channel_sid(body: &str) -> Result<String, GatewayError> {
    static SIGNALER_SID_REGEX: OnceLock<Regex> = OnceLock::new();
    SIGNALER_SID_REGEX
        .get_or_init(|| {
            Regex::new(r#"\["c","([^"]+)""#).expect("gemini canvas signaler sid regex must compile")
        })
        .captures(body)
        .and_then(|captures| captures.get(1))
        .map(|capture| capture.as_str().trim())
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| {
            GatewayError::server_error(
                "Gemini Canvas signaler open-channel response did not expose a SID.".to_string(),
            )
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_signaler_open_missing_sid")
        })
}

pub fn extract_signaler_long_poll_refresh_token(body: &str) -> Option<String> {
    static SIGNALER_REFRESH_TOKEN_REGEX: OnceLock<Regex> = OnceLock::new();
    SIGNALER_REFRESH_TOKEN_REGEX
        .get_or_init(|| {
            Regex::new(r#"null,null,\["([^"]+)"\]"#)
                .expect("gemini canvas signaler refresh token regex must compile")
        })
        .captures_iter(body)
        .filter_map(|captures| captures.get(1).map(|capture| capture.as_str().trim()))
        .find(|value| !value.is_empty() && value.chars().any(|ch| !ch.is_ascii_digit()))
        .map(ToString::to_string)
}

pub fn extract_signaler_long_poll_max_aid(body: &str) -> Option<u64> {
    static SIGNALER_AID_REGEX: OnceLock<Regex> = OnceLock::new();
    SIGNALER_AID_REGEX
        .get_or_init(|| {
            Regex::new(r#"\[\[(\d+),"#).expect("gemini canvas signaler aid regex must compile")
        })
        .captures_iter(body)
        .filter_map(|captures| {
            captures
                .get(1)
                .and_then(|value| value.as_str().parse::<u64>().ok())
        })
        .max()
}

pub fn extract_signaler_app_paths(body: &str) -> Vec<String> {
    static SIGNALER_APP_PATH_REGEX: OnceLock<Regex> = OnceLock::new();
    static SIGNALER_DECIMAL_APP_ID_REGEX: OnceLock<Regex> = OnceLock::new();
    let regex = SIGNALER_APP_PATH_REGEX.get_or_init(|| {
        Regex::new(r#"(?:/app/|c_)([0-9a-f]{8,})"#)
            .expect("gemini canvas signaler app path regex must compile")
    });
    let decimal_regex = SIGNALER_DECIMAL_APP_ID_REGEX.get_or_init(|| {
        Regex::new(r#"\["(\d{13,})"\]"#)
            .expect("gemini canvas signaler decimal app id regex must compile")
    });
    let mut paths = Vec::new();
    for capture in regex.captures_iter(body) {
        let Some(id) = capture.get(1).map(|value| value.as_str().trim()) else {
            continue;
        };
        if id.is_empty() {
            continue;
        }
        let path = format!("/app/{id}");
        if !paths.iter().any(|existing| existing == &path) {
            paths.push(path);
        }
    }
    for capture in decimal_regex.captures_iter(body) {
        let Some(id) = capture.get(1).map(|value| value.as_str().trim()) else {
            continue;
        };
        if id.is_empty() {
            continue;
        }
        let path = format!("/app/{id}");
        if !paths.iter().any(|existing| existing == &path) {
            paths.push(path);
        }
    }
    paths
}

pub fn extract_google_api_keys_from_page_blob(blob: &str) -> Vec<String> {
    static GOOGLE_API_KEY_REGEX: OnceLock<Regex> = OnceLock::new();
    let regex = GOOGLE_API_KEY_REGEX.get_or_init(|| {
        Regex::new(r"AIza[0-9A-Za-z\-_]{20,}").expect("google api key regex must compile")
    });
    let mut keys = Vec::new();
    for capture in regex.find_iter(blob) {
        let candidate = capture.as_str().trim();
        if candidate.is_empty() || keys.iter().any(|existing| existing == candidate) {
            continue;
        }
        keys.push(candidate.to_string());
    }
    keys
}

pub fn direct_http_referrer(base_url: &str, share_id: &str) -> String {
    format!("{}/share/{}", base_url.trim_end_matches('/'), share_id)
}

pub fn storage_state_to_pure_http_session(
    storage_state: &Value,
    target_url: &str,
    base_url: &str,
    auth_user: &str,
) -> Result<GeminiCanvasPureHttpSession, GatewayError> {
    let target_host = host_from_url(target_url).ok_or_else(|| {
        GatewayError::server_error("Gemini Canvas pure HTTP target URL was malformed.")
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_pure_http_bad_url")
    })?;
    let base_host = host_from_url(base_url).unwrap_or_else(|| "gemini.google.com".to_string());
    let target_path = path_from_url(target_url).unwrap_or_else(|| "/".to_string());
    let base_path = path_from_url(base_url).unwrap_or_else(|| "/".to_string());
    let target_scheme = scheme_from_url(target_url).unwrap_or("https");
    let base_scheme = scheme_from_url(base_url).unwrap_or("https");
    let cookies = storage_state
        .get("cookies")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            GatewayError::server_error(
                "Gemini Canvas browser-state object did not contain a Playwright cookies array.",
            )
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_pure_http_missing_cookies")
        })?;

    #[derive(Debug, Clone)]
    struct MatchedCookie {
        name: String,
        value: String,
        path: String,
        exact_host: bool,
        original_index: usize,
    }

    let mut matched_cookies: Vec<MatchedCookie> = Vec::new();
    for (index, cookie) in cookies.iter().enumerate() {
        let Some(name) = cookie.get("name").and_then(Value::as_str).map(str::trim) else {
            continue;
        };
        let Some(value) = cookie.get("value").and_then(Value::as_str).map(str::trim) else {
            continue;
        };
        if name.is_empty() || value.is_empty() {
            continue;
        }

        let matches_target = cookie_matches_url(cookie, &target_host, &target_path, target_scheme);
        let matches_base = cookie_matches_url(cookie, &base_host, &base_path, base_scheme);
        let matches_target_origin = cookie_matches_origin(cookie, &target_host, target_scheme);
        let matches_base_origin = cookie_matches_origin(cookie, &base_host, base_scheme);
        if !matches_target && !matches_base && !matches_target_origin && !matches_base_origin {
            continue;
        }

        let cookie_domain = cookie
            .get("domain")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default();
        let path = cookie
            .get("path")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("/")
            .to_string();
        matched_cookies.push(MatchedCookie {
            name: name.to_string(),
            value: value.to_string(),
            path,
            exact_host: !cookie_domain.starts_with('.')
                && cookie_domain.eq_ignore_ascii_case(&base_host),
            original_index: index,
        });
    }

    let is_gemini_google_target = target_host.contains("gemini.google.com")
        || base_host.contains("gemini.google.com")
        || target_host.contains("google.com")
        || base_host.contains("google.com");
    if is_gemini_google_target {
        for (index, cookie) in cookies.iter().enumerate() {
            let Some(name) = cookie.get("name").and_then(Value::as_str).map(str::trim) else {
                continue;
            };
            let Some(value) = cookie.get("value").and_then(Value::as_str).map(str::trim) else {
                continue;
            };
            if name.is_empty() || value.is_empty() {
                continue;
            }
            let cookie_domain = cookie
                .get("domain")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default();
            if !cookie_domain.contains("google.") && !cookie_domain.contains("gemini.google.com") {
                continue;
            }
            if matched_cookies
                .iter()
                .any(|entry| entry.name == name && entry.value == value)
            {
                continue;
            }
            let path = cookie
                .get("path")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("/")
                .to_string();
            matched_cookies.push(MatchedCookie {
                name: name.to_string(),
                value: value.to_string(),
                path,
                exact_host: false,
                original_index: index,
            });
        }
    }

    if matched_cookies.is_empty() && is_fixture_runtime_host(&target_host, &base_host) {
        for (index, cookie) in cookies.iter().enumerate() {
            let Some(name) = cookie.get("name").and_then(Value::as_str).map(str::trim) else {
                continue;
            };
            let Some(value) = cookie.get("value").and_then(Value::as_str).map(str::trim) else {
                continue;
            };
            if name.is_empty() || value.is_empty() {
                continue;
            }
            let cookie_domain = cookie
                .get("domain")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default();
            if !cookie_domain.contains("google.") && !cookie_domain.contains("gemini.google.com") {
                continue;
            }
            let path = cookie
                .get("path")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("/")
                .to_string();
            matched_cookies.push(MatchedCookie {
                name: name.to_string(),
                value: value.to_string(),
                path,
                exact_host: false,
                original_index: index,
            });
        }
    }

    matched_cookies.sort_by(|a, b| {
        b.path
            .len()
            .cmp(&a.path.len())
            .then_with(|| b.exact_host.cmp(&a.exact_host))
            .then_with(|| a.original_index.cmp(&b.original_index))
    });

    let sapisid = matched_cookies
        .iter()
        .find(|cookie| {
            matches!(
                cookie.name.as_str(),
                "__Secure-1PAPISID" | "__Secure-3PAPISID" | "SAPISID"
            )
        })
        .map(|cookie| cookie.value.clone())
        .ok_or_else(|| {
            GatewayError::service_unavailable(
                "Gemini Canvas browser-state did not include SAPISID-compatible Google cookies for pure HTTP signing.",
            )
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_pure_http_missing_sapisid")
        })?;
    if matched_cookies.is_empty() {
        return Err(GatewayError::service_unavailable(
            "Gemini Canvas browser-state did not include any cookies usable for pure HTTP replay.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_pure_http_missing_cookie_header"));
    }

    let cookie_header = matched_cookies
        .iter()
        .map(|cookie| format!("{}={}", cookie.name, cookie.value))
        .collect::<Vec<_>>()
        .join("; ");

    Ok(GeminiCanvasPureHttpSession {
        cookie_header,
        sapisid,
        auth_user: auth_user.trim().to_string(),
    })
}

pub fn pure_http_session_from_cookie_header(
    cookie_header: &str,
    auth_user: &str,
) -> Result<GeminiCanvasPureHttpSession, GatewayError> {
    let cookie_header = cookie_header.trim();
    if cookie_header.is_empty() {
        return Err(GatewayError::service_unavailable(
            "Gemini Canvas browser-state did not include any cookies usable for pure HTTP replay.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_pure_http_missing_cookie_header"));
    }

    let sapisid = cookie_header
        .split(';')
        .filter_map(|chunk| {
            let (name, value) = chunk.trim().split_once('=')?;
            let name = name.trim();
            let value = value.trim();
            if value.is_empty() {
                return None;
            }
            matches!(
                name,
                "__Secure-1PAPISID" | "__Secure-3PAPISID" | "SAPISID"
            )
            .then_some(value.to_string())
        })
        .next()
        .ok_or_else(|| {
            GatewayError::service_unavailable(
                "Gemini Canvas browser-state did not include SAPISID-compatible Google cookies for pure HTTP signing.",
            )
            .with_provider("gemini_canvas_compatible")
            .with_code("gemini_canvas_pure_http_missing_sapisid")
        })?;

    Ok(GeminiCanvasPureHttpSession {
        cookie_header: cookie_header.to_string(),
        sapisid,
        auth_user: auth_user.trim().to_string(),
    })
}

fn is_fixture_runtime_host(target_host: &str, base_host: &str) -> bool {
    [target_host, base_host].iter().any(|host| {
        host.eq_ignore_ascii_case("host.docker.internal")
            || host.eq_ignore_ascii_case("localhost")
            || host.eq_ignore_ascii_case("127.0.0.1")
    })
}

pub fn build_sapisid_authorization(
    sapisid: &str,
    origin: &str,
    timestamp_secs: i64,
) -> Result<String, GatewayError> {
    let sapisid = sapisid.trim();
    if sapisid.is_empty() {
        return Err(GatewayError::service_unavailable(
            "Gemini Canvas pure HTTP signing requires a non-empty SAPISID cookie.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_pure_http_missing_sapisid"));
    }
    let signing_input = format!("{timestamp_secs} {sapisid} {}", origin.trim());
    let digest = <sha1::Sha1 as sha1::Digest>::digest(signing_input.as_bytes());
    let hash = hex::encode(digest);
    Ok(format!(
        "SAPISIDHASH {timestamp_secs}_{hash} SAPISID1PHASH {timestamp_secs}_{hash} SAPISID3PHASH {timestamp_secs}_{hash}"
    ))
}

fn host_from_url(url: &str) -> Option<String> {
    let trimmed = url.trim();
    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let host = without_scheme
        .split(['/', '?', '#'])
        .next()?
        .split('@')
        .last()
        .unwrap_or_default()
        .split(':')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    (!host.is_empty()).then_some(host)
}

fn cookie_domain_matches(cookie_domain: &str, host: &str) -> bool {
    let raw_domain = cookie_domain.trim().to_ascii_lowercase();
    let domain = raw_domain.trim_start_matches('.');
    let host = host.trim().to_ascii_lowercase();
    if domain.is_empty() || host.is_empty() {
        return false;
    }
    if raw_domain.starts_with('.') {
        host == domain || host.ends_with(&format!(".{domain}"))
    } else {
        host == domain
    }
}

fn current_unix_timestamp_i64() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

fn scheme_from_url(url: &str) -> Option<&str> {
    let trimmed = url.trim();
    if trimmed.starts_with("https://") {
        Some("https")
    } else if trimmed.starts_with("http://") {
        Some("http")
    } else {
        None
    }
}

fn path_from_url(url: &str) -> Option<String> {
    let trimmed = url.trim();
    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let path = without_scheme
        .split_once('/')
        .map(|(_, rest)| {
            let path = rest.split(['?', '#']).next().unwrap_or_default();
            if path.is_empty() {
                "/".to_string()
            } else {
                format!("/{}", path)
            }
        })
        .unwrap_or_else(|| "/".to_string());
    Some(path)
}

fn cookie_path_matches(cookie_path: &str, request_path: &str) -> bool {
    let cookie_path = if cookie_path.trim().is_empty() {
        "/"
    } else {
        cookie_path.trim()
    };
    let request_path = if request_path.trim().is_empty() {
        "/"
    } else {
        request_path.trim()
    };

    if cookie_path == "/" {
        return true;
    }
    if request_path == cookie_path {
        return true;
    }
    if !request_path.starts_with(cookie_path) {
        return false;
    }

    cookie_path.ends_with('/')
        || request_path
            .as_bytes()
            .get(cookie_path.len())
            .is_some_and(|byte| *byte == b'/')
}

fn cookie_matches_url(cookie: &Value, host: &str, path: &str, scheme: &str) -> bool {
    if !cookie_matches_origin(cookie, host, scheme) {
        return false;
    }

    let cookie_path = cookie
        .get("path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("/");
    cookie_path_matches(cookie_path, path)
}

fn cookie_matches_origin(cookie: &Value, host: &str, scheme: &str) -> bool {
    let cookie_domain = cookie
        .get("domain")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    if cookie_domain.is_empty() || host.trim().is_empty() {
        return false;
    }

    if cookie
        .get("secure")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && !scheme.eq_ignore_ascii_case("https")
    {
        return false;
    }

    if let Some(expires) = cookie.get("expires").and_then(Value::as_f64) {
        let now_secs = current_unix_timestamp_i64() as f64;
        if expires > 0.0 && expires <= now_secs {
            return false;
        }
    }

    if !cookie_domain_matches(cookie_domain, host) {
        return false;
    }
    true
}

pub fn build_tts_request_body(req: &CanonicalRelayRequest, model: &str) -> Value {
    crate::protocol::gemini::api::build_tts_request_body(req, model)
}

pub fn extract_audio_from_generate_content_response(
    body: &Value,
) -> Result<GeminiCanvasAudio, GatewayError> {
    crate::protocol::gemini::api::extract_audio_from_generate_content_response(body)
}

pub fn extract_inline_image_from_generate_content_response(
    body: &Value,
) -> Result<GeminiCanvasImage, GatewayError> {
    crate::protocol::gemini::api::extract_inline_image_from_generate_content_response(body)
}

pub fn requested_tts_response_format(req: &CanonicalRelayRequest) -> Result<String, GatewayError> {
    gemini_canvas_modular::requested_tts_response_format(req)
}

pub fn build_audio_binary_response(
    req: &CanonicalRelayRequest,
    audio: &GeminiCanvasAudio,
) -> Result<(Vec<u8>, String), GatewayError> {
    gemini_canvas_modular::build_audio_binary_response(req, audio)
}

pub fn extract_stream_generate_media_assets(
    body: &str,
    operation: GeminiCanvasMediaOperation,
) -> Result<Vec<GeminiCanvasMediaAsset>, GatewayError> {
    let blocked_error = |details: String| {
        let mut error = GatewayError::service_unavailable(
            "Gemini Canvas image generation is unavailable for the current pure HTTP session or location.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_image_generation_unavailable");
        error.message = details;
        error
    };
    let preview = |text: &str, limit: usize, from_end: bool| {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return "<empty>".to_string();
        }
        let chars: Vec<char> = trimmed.chars().collect();
        if from_end {
            let start = chars.len().saturating_sub(limit);
            chars[start..].iter().collect::<String>()
        } else {
            chars.into_iter().take(limit).collect::<String>()
        }
    };
    let normalized = strip_xssi_prefix(body);
    let (frames, _remainder) = parse_response_frames(normalized);
    if frames.is_empty() {
        if operation == GeminiCanvasMediaOperation::Image
            && response_indicates_image_generation_unavailable(200, None, normalized)
        {
            return Err(blocked_error(format!(
                "Gemini Canvas image generation is unavailable for the current pure HTTP session or location.; body_head={}; body_tail={}",
                preview(normalized, 180, false),
                preview(normalized, 180, true)
            )));
        }
        let mut error = GatewayError::server_error(
            "Gemini Canvas StreamGenerate response did not contain any parseable frames.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_stream_generate_invalid_frame_response");
        error.message = format!(
            "{}; body_head={}; body_tail={}",
            error.message,
            preview(normalized, 180, false),
            preview(normalized, 180, true)
        );
        return Err(error);
    }

    let mut assets = Vec::new();
    let mut seen_urls = HashSet::new();
    for frame in &frames {
        collect_stream_generate_media_assets(frame, operation, &mut assets, &mut seen_urls);
    }
    if operation == GeminiCanvasMediaOperation::Music {
        prioritize_music_assets(&mut assets);
    }

    if assets.is_empty() {
        let debug_summary = summarize_stream_generate_media_debug_summary(&frames);
        if operation == GeminiCanvasMediaOperation::Image
            && response_indicates_image_generation_unavailable(200, None, normalized)
        {
            return Err(blocked_error(format!(
                "Gemini Canvas image generation is unavailable for the current pure HTTP session or location.; frame_count={}; {}",
                frames.len(),
                debug_summary
            )));
        }
        let code = match operation {
            GeminiCanvasMediaOperation::Image => {
                "gemini_canvas_stream_generate_missing_image_asset"
            }
            GeminiCanvasMediaOperation::Music => {
                "gemini_canvas_stream_generate_missing_music_asset"
            }
            GeminiCanvasMediaOperation::Video => {
                "gemini_canvas_stream_generate_missing_video_asset"
            }
        };
        let frame_preview = frames
            .iter()
            .take(3)
            .map(|frame| {
                serde_json::to_string(frame)
                    .map(|json| preview(&json, 220, false))
                    .unwrap_or_else(|_| "<unserializable-frame>".to_string())
            })
            .collect::<Vec<_>>()
            .join(" | ");
        let mut error = GatewayError::server_error(
            "Gemini Canvas StreamGenerate response did not include a usable media asset.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code(code);
        error.message = format!(
            "{}; frame_count={}; frame_preview={}; {}",
            error.message,
            frames.len(),
            frame_preview,
            debug_summary
        );
        return Err(error);
    }

    Ok(assets)
}

pub fn extract_page_blob_media_assets(
    body: &str,
    operation: GeminiCanvasMediaOperation,
) -> Result<Vec<GeminiCanvasMediaAsset>, GatewayError> {
    let normalized = normalize_page_blob_media_text(body);
    if operation == GeminiCanvasMediaOperation::Video
        && response_indicates_video_generation_quota_reached(&normalized)
    {
        return Err(GatewayError::service_unavailable(
            "Gemini Canvas video generation quota is currently exhausted.",
        )
        .with_provider("gemini_canvas_compatible")
        .with_code("gemini_canvas_video_quota_reached"));
    }
    let download_urls = extract_google_media_asset_urls(&normalized);
    if download_urls.is_empty() {
        return Err(build_page_blob_media_missing_error(&[], operation));
    }

    let mut assets = Vec::new();
    let mut seen_urls = HashSet::new();
    for url in download_urls.into_iter().rev() {
        if !seen_urls.insert(url.clone()) {
            continue;
        }
        if let Some(asset) = extract_page_blob_media_asset(&url, operation) {
            assets.push(asset);
        }
    }
    if operation == GeminiCanvasMediaOperation::Music {
        prioritize_music_assets(&mut assets);
    }

    if assets.is_empty() {
        return Err(build_page_blob_media_missing_error(
            &seen_urls.into_iter().collect::<Vec<_>>(),
            operation,
        ));
    }

    Ok(assets)
}

fn prioritize_music_assets(assets: &mut [GeminiCanvasMediaAsset]) {
    assets.sort_by_key(|asset| {
        if asset.kind == "audio" || asset.mime_type.starts_with("audio/") {
            0u8
        } else if asset.kind == "video" || asset.mime_type.starts_with("video/") {
            1u8
        } else {
            2u8
        }
    });
}

pub fn response_indicates_image_generation_unavailable(
    status: u16,
    content_type: Option<&str>,
    body: &str,
) -> bool {
    if !(200..300).contains(&status) {
        return false;
    }

    let normalized_content_type = content_type.unwrap_or_default().to_ascii_lowercase();
    let lower = body.to_ascii_lowercase();
    let text_like = normalized_content_type.is_empty()
        || normalized_content_type.contains("application/json")
        || normalized_content_type.contains("text/plain");
    let has_real_image_artifact = lower.contains("image_generation_content")
        && (lower.contains("googleusercontent")
            || lower.contains("gstatic")
            || lower.contains("blob:")
            || lower.contains("data:image/"));
    if has_real_image_artifact {
        return false;
    }

    let blocked_like = lower.contains("can't create it right now")
        || lower.contains("can't seem to create any")
        || lower.contains("can't create any for you")
        || (lower.contains("search for images") && lower.contains("can't create"))
        || lower.contains("are you signed in")
        || body.contains("您登录了吗")
        || body.contains("似乎无法为您创建任何图片")
        || body.contains("所在的地区尚未开通图片创建功能");
    text_like && blocked_like
}

pub fn extract_audio_from_tts_export_response(
    body: &str,
) -> Result<GeminiCanvasAudio, GatewayError> {
    gemini_canvas_modular::extract_audio_from_tts_export_response(body)
}

pub fn extract_stream_generate_locator(
    body: &str,
) -> Result<GeminiCanvasStreamGenerateLocator, GatewayError> {
    gemini_canvas_modular::extract_stream_generate_locator(body)
}

pub fn extract_stream_generate_response_id(body: &str) -> Result<String, GatewayError> {
    gemini_canvas_modular::extract_stream_generate_response_id(body)
}

pub fn stream_generate_indicates_image_edit_async_followup_ready(body: &str) -> bool {
    if extract_stream_generate_media_assets(body, GeminiCanvasMediaOperation::Image).is_ok() {
        return false;
    }
    if extract_stream_generate_locator(body).is_ok() {
        return true;
    }
    let normalized = strip_xssi_prefix(body);
    normalized.contains("\"37\":[")
        || normalized.contains("\\\"37\\\":[")
        || normalized.contains("\"18\":\"r_")
        || normalized.contains("\\\"18\\\":\\\"r_")
}

pub fn stream_generate_is_image_edit_short_ack(body: &str) -> bool {
    if extract_stream_generate_media_assets(body, GeminiCanvasMediaOperation::Image).is_ok() {
        return false;
    }
    let normalized = strip_xssi_prefix(body);
    normalized.contains("\"wrb.fr\",null,null,null,null,[13]")
        && normalized.contains("\"di\",")
        && normalized.contains("\"af.httprm\",")
}

pub fn stream_generate_parseable_frame_count(body: &str) -> usize {
    let normalized = strip_xssi_prefix(body);
    let (parsed_frames, _remainder) = parse_response_frames(normalized);
    if !parsed_frames.is_empty() {
        return parsed_frames.len();
    }
    parse_locator_response_envelopes(normalized).len()
}

pub fn extract_conversation_list_entries(body: &str) -> Vec<GeminiCanvasConversationListEntry> {
    gemini_canvas_modular::extract_conversation_list_entries(body)
}

pub fn extract_video_generation_job_id(body: &str) -> Option<String> {
    gemini_canvas_modular::extract_video_generation_job_id(body)
}

pub fn response_indicates_video_generation_pending(body: &str) -> bool {
    gemini_canvas_modular::response_indicates_video_generation_pending(body)
}

pub fn response_indicates_video_generation_quota_reached(body: &str) -> bool {
    gemini_canvas_modular::response_indicates_video_generation_quota_reached(body)
}

pub fn build_openai_images_response_from_urls(
    req: &CanonicalRelayRequest,
    prompt: &str,
    images: &[GeminiCanvasMediaAsset],
) -> Result<Value, GatewayError> {
    gemini_canvas_modular::build_openai_images_response_from_urls(req, prompt, images)
}

pub fn build_openai_images_response_from_bytes(
    req: &CanonicalRelayRequest,
    prompt: &str,
    images: &[GeminiCanvasImage],
) -> Result<Value, GatewayError> {
    gemini_canvas_modular::build_openai_images_response_from_bytes(req, prompt, images)
}

pub fn extract_images_from_imagen_predict_response(
    body: &Value,
) -> Result<Vec<GeminiCanvasImage>, GatewayError> {
    gemini_canvas_modular::extract_images_from_imagen_predict_response(body)
}

pub fn build_music_generation_response(
    model: &str,
    prompt: &str,
    asset: &GeminiCanvasMediaAsset,
    body_text: Option<&str>,
) -> Value {
    let mut value = crate::protocol::gemini::api::build_music_generation_response(
        model, prompt, asset, body_text,
    );
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "provider".to_string(),
            Value::String("gemini_canvas".to_string()),
        );
    }
    value
}

pub fn build_music_generation_accepted_response(
    model: &str,
    prompt: &str,
    conversation_id: Option<&str>,
    response_id: Option<&str>,
    app_path: Option<&str>,
    duration_seconds: Option<f64>,
    body_text: Option<&str>,
) -> Value {
    let mut value = crate::protocol::gemini::api::build_music_generation_accepted_response(
        model,
        prompt,
        conversation_id,
        response_id,
        app_path,
        duration_seconds,
        body_text,
    );
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "provider".to_string(),
            Value::String("gemini_canvas".to_string()),
        );
    }
    value
}

pub fn build_video_generation_response(
    model: &str,
    prompt: &str,
    asset: &GeminiCanvasMediaAsset,
    body_text: Option<&str>,
) -> Value {
    let mut value = crate::protocol::gemini::api::build_video_generation_response(
        model, prompt, asset, body_text,
    );
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "provider".to_string(),
            Value::String("gemini_canvas".to_string()),
        );
    }
    value
}

pub fn build_video_generation_accepted_response(
    model: &str,
    prompt: &str,
    conversation_id: Option<&str>,
    response_id: Option<&str>,
    app_path: Option<&str>,
    job_id: Option<&str>,
    body_text: Option<&str>,
) -> Value {
    let mut value = crate::protocol::gemini::api::build_video_generation_accepted_response(
        model,
        prompt,
        conversation_id,
        response_id,
        app_path,
        job_id,
        body_text,
    );
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "provider".to_string(),
            Value::String("gemini_canvas".to_string()),
        );
    }
    value
}

fn normalize_prompt_request(
    body: Value,
    endpoint_kind: EndpointKind,
    default_model: &str,
    missing_message: &str,
    missing_code: &str,
) -> Result<CanonicalRelayRequest, GatewayError> {
    let body_obj = body.as_object().ok_or_else(|| {
        GatewayError::bad_request("Media generation request body must be a JSON object")
    })?;

    if body_obj
        .get("stream")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
    {
        return Err(GatewayError::bad_request(
            "Media generation endpoints do not support `stream: true`",
        )
        .with_code("media_streaming_not_supported"));
    }

    let prompt = read_prompt_body(body_obj)
        .ok_or_else(|| GatewayError::bad_request(missing_message).with_code(missing_code))?;

    let requested_model = body_obj
        .get("model")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| Some(default_model.to_string()));

    let explicit_session_key = body_obj
        .get("user")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    Ok(CanonicalRelayRequest {
        protocol_family: ProtocolFamily::OpenAi,
        endpoint_kind,
        requested_model,
        stream: false,
        messages: vec![CanonicalMessage {
            role: MessageRole::User,
            content: vec![ContentPart::Text { text: prompt }],
            name: None,
            tool_call_id: None,
            tool_calls: vec![],
        }],
        tools: vec![],
        tool_choice: None,
        reasoning: None,
        metadata: None,
        raw_body: body,
        previous_response_id: None,
        explicit_session_key,
        extra: HashMap::new(),
    })
}

fn read_prompt_body(map: &serde_json::Map<String, Value>) -> Option<String> {
    for field in ["prompt", "input", "lyrics"] {
        if let Some(text) = map
            .get(field)
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(text.to_string());
        }
    }

    let parts = map.get("parts")?.as_array()?;
    let mut texts = Vec::new();
    for part in parts {
        match part {
            Value::String(text) => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    texts.push(trimmed.to_string());
                }
            }
            Value::Object(part_map) => {
                if let Some(text) = part_map
                    .get("content")
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    texts.push(text.to_string());
                }
            }
            _ => {}
        }
    }

    if texts.is_empty() {
        None
    } else {
        Some(texts.join("\n"))
    }
}

pub fn video_body_indicates_music_modality_mismatch(body_text: &str) -> bool {
    let lower = body_text.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return false;
    }

    lower.contains("generated_music_content")
        || lower.contains("create an original music clip")
        || lower.contains("music request:")
        || lower.contains("gemini.google.com/music")
        || lower.contains("\"object\":\"music.generation\"")
}

fn strip_xssi_prefix(body: &str) -> &str {
    body.trim_start()
        .strip_prefix(")]}'")
        .map(str::trim_start)
        .unwrap_or(body)
}

fn looks_like_stream_generate_id(value: &str, prefix: &str) -> bool {
    let candidate = value.trim();
    candidate.starts_with(prefix)
        && candidate.len() > prefix.len()
        && candidate[prefix.len()..]
            .chars()
            .all(|ch| ch.is_ascii_hexdigit())
}

fn parse_locator_response_envelopes(content: &str) -> Vec<Value> {
    let content = content.trim();
    if content.is_empty() {
        return Vec::new();
    }

    if let Ok(parsed) = serde_json::from_str::<Value>(content) {
        return match parsed {
            Value::Array(items) => items,
            other => vec![other],
        };
    }

    let mut collected = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(parsed) = serde_json::from_str::<Value>(line) {
            match parsed {
                Value::Array(items) => collected.extend(items),
                other => collected.push(other),
            }
        }
    }

    collected
}

fn is_cjk_unified_ideograph(ch: char) -> bool {
    matches!(ch as u32, 0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0x20000..=0x2A6DF)
}

fn is_hiragana_or_katakana(ch: char) -> bool {
    matches!(ch as u32, 0x3040..=0x30FF | 0x31F0..=0x31FF)
}

fn is_hangul(ch: char) -> bool {
    matches!(ch as u32, 0x1100..=0x11FF | 0x3130..=0x318F | 0xAC00..=0xD7AF)
}

fn is_cyrillic(ch: char) -> bool {
    matches!(ch as u32, 0x0400..=0x04FF | 0x0500..=0x052F)
}

fn parse_response_frames(content: &str) -> (Vec<Value>, String) {
    let mut consumed_chars = 0usize;
    let mut frames = Vec::new();
    while consumed_chars < content.len() {
        let mut index = consumed_chars;
        while let Some(ch) = content[index..].chars().next() {
            if ch.is_whitespace() {
                index += ch.len_utf8();
            } else {
                break;
            }
            if index >= content.len() {
                break;
            }
        }
        if index >= content.len() {
            consumed_chars = index;
            break;
        }

        let mut digit_end = index;
        while let Some(ch) = content[digit_end..].chars().next() {
            if ch.is_ascii_digit() {
                digit_end += ch.len_utf8();
            } else {
                break;
            }
            if digit_end >= content.len() {
                break;
            }
        }
        if digit_end == index {
            break;
        }
        let line_ending = if content[digit_end..].starts_with("\r\n") {
            2
        } else if content[digit_end..].starts_with('\n') {
            1
        } else {
            break;
        };
        let length = match content[index..digit_end].parse::<usize>() {
            Ok(value) => value,
            Err(_) => break,
        };
        let start_content = digit_end + line_ending;
        let Some((end_pos, parsed)) = parse_response_frame_chunk(content, start_content, length)
        else {
            break;
        };
        consumed_chars = end_pos;
        match parsed {
            Value::Array(items) => frames.extend(items),
            other => frames.push(other),
        }
    }

    (frames, content[consumed_chars..].to_string())
}

fn parse_response_frame_chunk(
    content: &str,
    start: usize,
    expected_length: usize,
) -> Option<(usize, Value)> {
    for end in [
        utf16_end_index(content, start, expected_length),
        fallback_line_json_end_index(content, start),
    ]
    .into_iter()
    .flatten()
    {
        let chunk = content[start..end].trim();
        if chunk.is_empty() {
            continue;
        }
        if let Ok(parsed) = serde_json::from_str::<Value>(chunk) {
            return Some((end, parsed));
        }
    }

    None
}

fn fallback_line_json_end_index(content: &str, start: usize) -> Option<usize> {
    if start >= content.len() {
        return None;
    }
    let remainder = &content[start..];
    let line_end_offset = remainder.find('\n').unwrap_or(remainder.len());
    let mut end = start + line_end_offset;
    if end > start && content.as_bytes().get(end.wrapping_sub(1)) == Some(&b'\r') {
        end -= 1;
    }
    let chunk = content[start..end].trim();
    if chunk.is_empty() {
        return None;
    }
    serde_json::from_str::<Value>(chunk).ok().map(|_| end)
}

fn utf16_end_index(content: &str, start: usize, units: usize) -> Option<usize> {
    let mut consumed_units = 0usize;
    for (offset, ch) in content[start..].char_indices() {
        let width = ch.len_utf16();
        if consumed_units + width > units {
            break;
        }
        consumed_units += width;
        if consumed_units == units {
            return Some(start + offset + ch.len_utf8());
        }
    }
    None
}

fn collect_stream_generate_media_assets(
    value: &Value,
    operation: GeminiCanvasMediaOperation,
    assets: &mut Vec<GeminiCanvasMediaAsset>,
    seen_urls: &mut HashSet<String>,
) {
    if let Some(mut extracted) = extract_media_assets_from_candidate_data(value, operation) {
        extracted.retain(|asset| seen_urls.insert(asset.url.clone()));
        assets.extend(extracted);
        return;
    }

    match value {
        Value::Array(items) => {
            for item in items {
                collect_stream_generate_media_assets(item, operation, assets, seen_urls);
            }
        }
        Value::Object(map) => {
            for item in map.values() {
                collect_stream_generate_media_assets(item, operation, assets, seen_urls);
            }
        }
        Value::String(text) => {
            if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                collect_stream_generate_media_assets(&parsed, operation, assets, seen_urls);
            }
        }
        _ => {}
    }
}

fn summarize_stream_generate_media_debug_summary(frames: &[Value]) -> String {
    let mut ids = Vec::new();
    let mut ids_seen = HashSet::new();
    let mut urls = Vec::new();
    let mut urls_seen = HashSet::new();
    let mut texts = Vec::new();
    let mut texts_seen = HashSet::new();

    for frame in frames {
        collect_stream_generate_media_debug_values(
            frame,
            &mut ids,
            &mut ids_seen,
            &mut urls,
            &mut urls_seen,
            &mut texts,
            &mut texts_seen,
        );
    }

    format!(
        "detected_ids={}; detected_urls={}; detected_text={}",
        if ids.is_empty() {
            "<none>".to_string()
        } else {
            ids.join(",")
        },
        if urls.is_empty() {
            "<none>".to_string()
        } else {
            urls.join(",")
        },
        if texts.is_empty() {
            "<none>".to_string()
        } else {
            texts.join(" | ")
        }
    )
}

fn collect_stream_generate_media_debug_values(
    value: &Value,
    ids: &mut Vec<String>,
    ids_seen: &mut HashSet<String>,
    urls: &mut Vec<String>,
    urls_seen: &mut HashSet<String>,
    texts: &mut Vec<String>,
    texts_seen: &mut HashSet<String>,
) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_stream_generate_media_debug_values(
                    item, ids, ids_seen, urls, urls_seen, texts, texts_seen,
                );
            }
        }
        Value::Object(map) => {
            for item in map.values() {
                collect_stream_generate_media_debug_values(
                    item, ids, ids_seen, urls, urls_seen, texts, texts_seen,
                );
            }
        }
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return;
            }

            if (looks_like_stream_generate_id(trimmed, "r_")
                || looks_like_stream_generate_id(trimmed, "c_")
                || (trimmed.starts_with("rc_")
                    && trimmed.len() >= 6
                    && trimmed[3..].chars().all(|ch| ch.is_ascii_hexdigit())))
                && ids_seen.insert(trimmed.to_string())
                && ids.len() < 12
            {
                ids.push(trimmed.to_string());
            }

            if (trimmed.starts_with("https://")
                || trimmed.starts_with("http://")
                || trimmed.starts_with("data:image/")
                || trimmed.starts_with("data:audio/")
                || trimmed.starts_with("data:video/")
                || trimmed.starts_with("blob:"))
                && urls_seen.insert(trimmed.to_string())
                && urls.len() < 12
            {
                urls.push(trimmed.to_string());
            }

            if trimmed.len() >= 24
                && trimmed.contains(' ')
                && trimmed.chars().any(|ch| ch.is_ascii_alphabetic())
                && !trimmed.starts_with('[')
                && !trimmed.starts_with('{')
            {
                let compact = if trimmed.chars().count() > 140 {
                    format!(
                        "{}...(truncated)",
                        trimmed.chars().take(140).collect::<String>()
                    )
                } else {
                    trimmed.to_string()
                };
                if texts_seen.insert(compact.clone()) && texts.len() < 8 {
                    texts.push(compact);
                }
            }

            if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
                collect_stream_generate_media_debug_values(
                    &parsed, ids, ids_seen, urls, urls_seen, texts, texts_seen,
                );
            }
        }
        _ => {}
    }
}

fn extract_media_assets_from_candidate_data(
    value: &Value,
    operation: GeminiCanvasMediaOperation,
) -> Option<Vec<GeminiCanvasMediaAsset>> {
    match operation {
        GeminiCanvasMediaOperation::Image => extract_image_assets_from_candidate_data(value),
        GeminiCanvasMediaOperation::Music => {
            extract_music_asset_from_candidate_data(value).map(|asset| vec![asset])
        }
        GeminiCanvasMediaOperation::Video => {
            extract_video_asset_from_candidate_data(value).map(|asset| vec![asset])
        }
    }
}

fn extract_image_assets_from_candidate_data(value: &Value) -> Option<Vec<GeminiCanvasMediaAsset>> {
    let mut assets = Vec::new();
    if let Some(gen_images) = get_nested_value(value, &[12, 7, 0])
        .or_else(|| get_nested_value(value, &[7, 0]))
        .and_then(Value::as_array)
    {
        for gen_img_data in gen_images {
            if let Some(asset) = extract_image_asset_from_generated_image_data(gen_img_data) {
                assets.push(asset);
            }
        }
    }

    if let Some(image_to_image) = value
        .as_array()
        .and_then(|items| items.get(12))
        .and_then(|item| item.as_array())
        .and_then(|items| items.first())
        .and_then(|first| first.get("8"))
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(Value::as_array)
        .or_else(|| {
            value
                .as_array()
                .and_then(|items| items.first())
                .and_then(|first| first.get("8"))
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(Value::as_array)
        })
    {
        for gen_img_data in image_to_image {
            if let Some(asset) = extract_image_asset_from_generated_image_data(gen_img_data) {
                assets.push(asset);
            }
        }
    }

    if assets.is_empty() {
        None
    } else {
        Some(assets)
    }
}

fn extract_video_asset_from_candidate_data(value: &Value) -> Option<GeminiCanvasMediaAsset> {
    get_nested_value(value, &[12, 59, 0, 0, 0])
        .or_else(|| get_nested_value(value, &[59, 0, 0, 0]))
        .and_then(extract_video_asset_from_video_info)
        .or_else(|| {
            extract_media_asset_from_value_url_scan(value, GeminiCanvasMediaOperation::Video)
        })
}

fn extract_music_asset_from_candidate_data(value: &Value) -> Option<GeminiCanvasMediaAsset> {
    let direct_path = get_nested_value(value, &[12, 86])
        .or_else(|| get_nested_value(value, &[86]))
        .or_else(|| get_nested_value(value, &[12, 87]))
        .or_else(|| get_nested_value(value, &[87]));
    if let Some(media_data) = direct_path {
        let mp3_list = get_nested_value(media_data, &[0, 1, 7])
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mp4_list = get_nested_value(media_data, &[1, 1, 7])
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let mp3_url = mp3_list
            .get(1)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(str::to_string);
        let mp4_url = mp4_list
            .get(1)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(str::to_string);

        if let Some(url) = mp3_url {
            return Some(GeminiCanvasMediaAsset {
                kind: "audio".to_string(),
                mime_type: infer_audio_mime_type(&url),
                url,
                download_token: None,
                body_base64: None,
                alt: None,
                width: None,
                height: None,
                duration_seconds: None,
            });
        }
        if let Some(url) = mp4_url {
            return Some(GeminiCanvasMediaAsset {
                kind: "video".to_string(),
                mime_type: infer_video_mime_type(&url),
                url,
                download_token: None,
                body_base64: None,
                alt: None,
                width: None,
                height: None,
                duration_seconds: None,
            });
        }
    }

    extract_media_asset_from_value_url_scan(value, GeminiCanvasMediaOperation::Music)
}

fn extract_image_asset_from_generated_image_data(value: &Value) -> Option<GeminiCanvasMediaAsset> {
    let url = get_nested_value(value, &[0, 3, 3])
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|entry| !entry.is_empty())?
        .to_string();
    let download_token = get_nested_value(value, &[0, 3, 5])
        .or_else(|| get_nested_value(value, &[0, 3, 4]))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string);
    let alt = get_nested_value(value, &[0, 3, 2])
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string);

    Some(GeminiCanvasMediaAsset {
        kind: "image".to_string(),
        mime_type: infer_image_mime_type(&url),
        url,
        download_token,
        body_base64: None,
        alt,
        width: None,
        height: None,
        duration_seconds: None,
    })
}

fn extract_page_blob_media_asset(
    url: &str,
    operation: GeminiCanvasMediaOperation,
) -> Option<GeminiCanvasMediaAsset> {
    let mime_type = match operation {
        GeminiCanvasMediaOperation::Image => infer_image_mime_type(url),
        GeminiCanvasMediaOperation::Music | GeminiCanvasMediaOperation::Video => {
            infer_mime_type(url, "")
        }
    };
    let (kind, mime_type) = match operation {
        GeminiCanvasMediaOperation::Image if mime_type.starts_with("image/") => {
            ("image".to_string(), mime_type)
        }
        GeminiCanvasMediaOperation::Music if mime_type.starts_with("audio/") => {
            ("audio".to_string(), mime_type)
        }
        GeminiCanvasMediaOperation::Music if mime_type.starts_with("video/") => {
            ("video".to_string(), mime_type)
        }
        GeminiCanvasMediaOperation::Video if mime_type.starts_with("video/") => {
            ("video".to_string(), mime_type)
        }
        _ => return None,
    };

    Some(GeminiCanvasMediaAsset {
        kind,
        url: url.to_string(),
        mime_type,
        download_token: None,
        body_base64: None,
        alt: None,
        width: None,
        height: None,
        duration_seconds: None,
    })
}

fn build_page_blob_media_missing_error(
    download_urls: &[String],
    operation: GeminiCanvasMediaOperation,
) -> GatewayError {
    let code = match operation {
        GeminiCanvasMediaOperation::Image => "gemini_canvas_page_missing_image_asset",
        GeminiCanvasMediaOperation::Music => "gemini_canvas_page_missing_music_asset",
        GeminiCanvasMediaOperation::Video => "gemini_canvas_page_missing_video_asset",
    };
    let candidate_filenames = download_urls
        .iter()
        .take(6)
        .filter_map(|url| extract_download_filename_from_url(url))
        .collect::<Vec<_>>();
    let filenames_preview = if candidate_filenames.is_empty() {
        "<none>".to_string()
    } else {
        candidate_filenames.join("|")
    };

    let mut error =
        GatewayError::server_error("Gemini Canvas page blob did not include a usable media asset.")
            .with_provider("gemini_canvas_compatible")
            .with_code(code);
    error.message = format!(
        "{}; download_candidate_count={}; candidate_filenames={}",
        error.message,
        download_urls.len(),
        filenames_preview
    );
    error
}

fn extract_download_filename_from_url(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    for (key, value) in parsed.query_pairs() {
        if key == "filename" {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }

    parsed
        .path_segments()
        .and_then(|segments| segments.last())
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(str::to_string)
}

fn extract_google_media_asset_urls(body: &str) -> Vec<String> {
    static GOOGLE_MEDIA_URL_REGEX: OnceLock<Regex> = OnceLock::new();
    let regex = GOOGLE_MEDIA_URL_REGEX.get_or_init(|| {
        Regex::new(
            r#"https?://(?:contribution\.usercontent\.google\.com/download\?[^"'<>\\\s]+|(?:lh3\.googleusercontent\.com|work\.fife\.usercontent\.google\.com)/(?:gg(?:-dl)?|rd-gg-dl|rd-ogw)/[^"'<>\\\s]+)"#,
        )
        .expect("google media asset url regex must compile")
    });

    regex
        .find_iter(body)
        .map(|matched| matched.as_str().trim().to_string())
        .collect()
}

fn normalize_page_blob_media_text(body: &str) -> String {
    body.replace("\\u003d", "=")
        .replace("\\u0026", "&")
        .replace("\\u003a", ":")
        .replace("\\u002f", "/")
        .replace("\\u003f", "?")
        .replace("\\u0025", "%")
        .replace("\\/", "/")
        .replace("&amp;", "&")
}

fn extract_video_asset_from_video_info(value: &Value) -> Option<GeminiCanvasMediaAsset> {
    let urls = get_nested_value(value, &[0, 7])?.as_array()?;
    let thumbnail = urls
        .first()
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string);
    let url = urls
        .get(1)
        .or_else(|| urls.first())?
        .as_str()
        .map(str::trim)
        .filter(|entry| !entry.is_empty())?
        .to_string();

    Some(GeminiCanvasMediaAsset {
        kind: "video".to_string(),
        mime_type: infer_video_mime_type(&url),
        url,
        download_token: None,
        body_base64: None,
        alt: thumbnail,
        width: None,
        height: None,
        duration_seconds: None,
    })
}

fn extract_media_asset_from_value_url_scan(
    value: &Value,
    operation: GeminiCanvasMediaOperation,
) -> Option<GeminiCanvasMediaAsset> {
    let mut urls = Vec::new();
    let mut seen = HashSet::new();
    collect_candidate_media_urls(value, &mut urls, &mut seen);
    select_best_media_asset_from_urls(&urls, operation)
}

fn collect_candidate_media_urls(value: &Value, urls: &mut Vec<String>, seen: &mut HashSet<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_candidate_media_urls(item, urls, seen);
            }
        }
        Value::Object(map) => {
            for item in map.values() {
                collect_candidate_media_urls(item, urls, seen);
            }
        }
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.starts_with("https://") && seen.insert(trimmed.to_string()) {
                urls.push(trimmed.to_string());
            } else if trimmed.starts_with('[') || trimmed.starts_with('{') {
                if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
                    collect_candidate_media_urls(&parsed, urls, seen);
                }
            }
        }
        _ => {}
    }
}

fn select_best_media_asset_from_urls(
    urls: &[String],
    operation: GeminiCanvasMediaOperation,
) -> Option<GeminiCanvasMediaAsset> {
    let mut best: Option<(i32, GeminiCanvasMediaAsset)> = None;
    for url in urls {
        let Some((rank, asset)) = rank_media_asset_candidate(url, operation) else {
            continue;
        };
        match &best {
            Some((best_rank, _)) if rank <= *best_rank => {}
            _ => best = Some((rank, asset)),
        }
    }
    best.map(|(_, asset)| asset)
}

fn rank_media_asset_candidate(
    url: &str,
    operation: GeminiCanvasMediaOperation,
) -> Option<(i32, GeminiCanvasMediaAsset)> {
    let trimmed = url.trim();
    if trimmed.is_empty()
        || trimmed.starts_with("blob:")
        || trimmed.starts_with("data:")
        || trimmed.contains("video_gen_chip")
        || trimmed.contains("generated_video_content")
    {
        return None;
    }

    let mime_type = infer_mime_type(trimmed, "");
    let lower = trimmed.to_ascii_lowercase();
    let host_bonus = if lower.contains("contribution.usercontent.google.com") {
        40
    } else if lower.contains("work.fife.usercontent.google.com/rd-gg-dl/") {
        30
    } else if lower.contains("lh3.googleusercontent.com/rd-ogw/") {
        28
    } else if lower.contains("lh3.googleusercontent.com/rd-gg-dl/") {
        25
    } else if lower.contains("lh3.googleusercontent.com/gg-dl/") {
        22
    } else if lower.contains("lh3.googleusercontent.com/gg/") {
        18
    } else {
        0
    };

    match operation {
        GeminiCanvasMediaOperation::Image => {
            let image_like = mime_type.starts_with("image/")
                || lower.contains("lh3.googleusercontent.com/rd-ogw/")
                || lower.contains("lh3.googleusercontent.com/gg-dl/")
                || lower.contains("lh3.googleusercontent.com/gg/")
                || lower.contains("contribution.usercontent.google.com/download?");
            if !image_like {
                return None;
            }
            Some((
                100 + host_bonus,
                GeminiCanvasMediaAsset {
                    kind: "image".to_string(),
                    mime_type: if mime_type.is_empty() {
                        infer_image_mime_type(trimmed)
                    } else {
                        mime_type
                    },
                    url: trimmed.to_string(),
                    download_token: None,
                    body_base64: None,
                    alt: None,
                    width: None,
                    height: None,
                    duration_seconds: None,
                },
            ))
        }
        GeminiCanvasMediaOperation::Music => {
            let (kind, base_rank) = if mime_type.starts_with("audio/") {
                ("audio".to_string(), 100)
            } else if mime_type.starts_with("video/") {
                ("video".to_string(), 80)
            } else {
                return None;
            };
            Some((
                base_rank + host_bonus,
                GeminiCanvasMediaAsset {
                    kind,
                    mime_type: if mime_type.is_empty() {
                        if lower.contains(".mp3") {
                            "audio/mpeg".to_string()
                        } else {
                            "video/mp4".to_string()
                        }
                    } else {
                        mime_type
                    },
                    url: trimmed.to_string(),
                    download_token: None,
                    body_base64: None,
                    alt: None,
                    width: None,
                    height: None,
                    duration_seconds: None,
                },
            ))
        }
        GeminiCanvasMediaOperation::Video => {
            if !mime_type.starts_with("video/") {
                return None;
            }
            Some((
                100 + host_bonus,
                GeminiCanvasMediaAsset {
                    kind: "video".to_string(),
                    mime_type: if mime_type.is_empty() {
                        "video/mp4".to_string()
                    } else {
                        mime_type
                    },
                    url: trimmed.to_string(),
                    download_token: None,
                    body_base64: None,
                    alt: None,
                    width: None,
                    height: None,
                    duration_seconds: None,
                },
            ))
        }
    }
}

fn get_nested_value<'a>(value: &'a Value, path: &[usize]) -> Option<&'a Value> {
    let mut current = value;
    for index in path {
        if let Some(items) = current.as_array() {
            current = items.get(*index)?;
            continue;
        }
        if let Some(map) = current.as_object() {
            current = map.get(&index.to_string())?;
            continue;
        }
        return None;
    }
    Some(current)
}

fn infer_image_mime_type(url: &str) -> String {
    infer_mime_type(url, "image/png")
}

fn infer_audio_mime_type(url: &str) -> String {
    infer_mime_type(url, "audio/mpeg")
}

fn infer_video_mime_type(url: &str) -> String {
    infer_mime_type(url, "video/mp4")
}

fn infer_mime_type(url: &str, default_mime_type: &str) -> String {
    let lower = url.trim().to_ascii_lowercase();
    if lower.starts_with("data:image/") {
        return lower
            .split_once(';')
            .map(|(mime, _)| mime.to_string())
            .unwrap_or_else(|| default_mime_type.to_string());
    }
    if lower.starts_with("data:audio/") {
        return lower
            .split_once(';')
            .map(|(mime, _)| mime.to_string())
            .unwrap_or_else(|| default_mime_type.to_string());
    }
    if lower.starts_with("data:video/") {
        return lower
            .split_once(';')
            .map(|(mime, _)| mime.to_string())
            .unwrap_or_else(|| default_mime_type.to_string());
    }

    if lower.contains(".png") {
        return "image/png".to_string();
    }
    if lower.contains(".jpg") || lower.contains(".jpeg") {
        return "image/jpeg".to_string();
    }
    if lower.contains(".webp") {
        return "image/webp".to_string();
    }
    if lower.contains(".gif") {
        return "image/gif".to_string();
    }
    if lower.contains(".mp3") {
        return "audio/mpeg".to_string();
    }
    if lower.contains(".wav") {
        return "audio/wav".to_string();
    }
    if lower.contains(".ogg") || lower.contains(".opus") {
        return "audio/ogg".to_string();
    }
    if lower.contains(".mp4") {
        return "video/mp4".to_string();
    }
    if lower.contains(".webm") {
        return "video/webm".to_string();
    }

    default_mime_type.to_string()
}

fn read_optional_string(value: &Value, keys: &[&str]) -> Option<String> {
    let map = value.as_object()?;
    for key in keys {
        if let Some(value) = map.get(*key).and_then(|entry| entry.as_str()) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn read_optional_hash_string(
    map: &std::collections::HashMap<String, Value>,
    keys: &[&str],
) -> Option<String> {
    for key in keys {
        if let Some(value) = map.get(*key).and_then(|entry| entry.as_str()) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn collect_non_empty_strings_from_slice(values: &[Value]) -> Vec<String> {
    let mut collected = Vec::new();
    for entry in values {
        let Some(candidate) = entry
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        if collected.iter().any(|existing| existing == candidate) {
            continue;
        }
        collected.push(candidate.to_string());
    }
    collected
}

fn read_optional_hash_string_array_values(
    map: &std::collections::HashMap<String, Value>,
) -> Vec<String> {
    ["apiKeys", "api_keys"]
        .iter()
        .find_map(|key| {
            map.get(*key)
                .and_then(Value::as_array)
                .map(|values| collect_non_empty_strings_from_slice(values))
        })
        .unwrap_or_default()
}

fn read_optional_string_array_values(value: &Value, keys: &[&str]) -> Vec<String> {
    let Some(map) = value.as_object() else {
        return Vec::new();
    };
    keys.iter()
        .find_map(|key| {
            map.get(*key)
                .and_then(Value::as_array)
                .map(|values| collect_non_empty_strings_from_slice(values))
        })
        .unwrap_or_default()
}

fn extend_unique_strings(target: &mut Vec<String>, values: Vec<String>) {
    for candidate in values {
        if target.iter().any(|existing| existing == &candidate) {
            continue;
        }
        target.push(candidate);
    }
}

fn extract_google_api_keys_from_hash_map(
    map: &std::collections::HashMap<String, Value>,
) -> Vec<String> {
    let mut keys = Vec::new();
    if let Some(primary) = read_optional_hash_string(
        map,
        &["googleApiKey", "google_api_key", "apiKey", "api_key"],
    ) {
        keys.push(primary);
    }
    extend_unique_strings(&mut keys, read_optional_hash_string_array_values(map));
    keys
}

fn extract_google_api_keys_from_value(value: &Value) -> Vec<String> {
    let mut keys = Vec::new();
    if let Some(primary) = read_optional_string(
        value,
        &["googleApiKey", "google_api_key", "apiKey", "api_key"],
    ) {
        keys.push(primary);
    }
    extend_unique_strings(
        &mut keys,
        read_optional_string_array_values(value, &["apiKeys", "api_keys"]),
    );
    if let Some(config) = value.as_object().and_then(|map| {
        map.get("firebaseConfig")
            .or_else(|| map.get("firebase_config"))
    }) {
        if let Some(primary) = read_optional_string(config, &["apiKey", "api_key"]) {
            extend_unique_strings(&mut keys, vec![primary]);
        }
        extend_unique_strings(
            &mut keys,
            read_optional_string_array_values(config, &["apiKeys", "api_keys"]),
        );
    }
    keys
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::canonical::{CanonicalTool, CanonicalToolCall};

    fn pure_http_mode_payload(mode: Option<&str>) -> ProviderAccountPayload {
        let mut extra_body = HashMap::new();
        if let Some(value) = mode {
            extra_body.insert("pureHttpMode".to_string(), Value::String(value.to_string()));
        }

        ProviderAccountPayload {
            adapter: "gemini_canvas_compatible".to_string(),
            base_url: "https://gemini.google.com".to_string(),
            api_key: String::new(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: Some(
                "credential-runtime/gemini-canvas-profile/test/user-data".to_string(),
            ),
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: Some(GEMINI_CANVAS_DEFAULT_MODEL.to_string()),
            headers: HashMap::new(),
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: None,
            chat_completions_path: None,
            completions_path: None,
            embeddings_path: None,
            audio_transcriptions_path: None,
            audio_speech_path: None,
            messages_path: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            extra_body: Some(extra_body),
            session_auth: None,
            keepalive: None,
        }
    }

    fn make_request(endpoint_kind: EndpointKind) -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind,
            requested_model: Some(GEMINI_CANVAS_DEFAULT_MODEL.to_string()),
            stream: false,
            messages: vec![],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({ "input": "say hello" }),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        }
    }

    #[test]
    fn runtime_reads_state_key_and_default_share_id() {
        let payload = ProviderAccountPayload {
            adapter: "gemini_canvas_compatible".to_string(),
            base_url: "https://gemini.google.com".to_string(),
            api_key: "AIzaPayloadKeyZero".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: Some(
                "credential-runtime/gemini-canvas-profile/test/user-data".to_string(),
            ),
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: Some(GEMINI_CANVAS_DEFAULT_MODEL.to_string()),
            headers: HashMap::new(),
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: None,
            chat_completions_path: None,
            completions_path: None,
            embeddings_path: None,
            audio_transcriptions_path: None,
            audio_speech_path: None,
            messages_path: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            extra_body: Some(HashMap::new()),
            session_auth: None,
            keepalive: None,
        };

        let runtime = runtime_from_payload(&payload).unwrap();
        assert_eq!(
            runtime.runtime_state_object_key,
            "credential-runtime/gemini-canvas-profile/test/user-data"
        );
        assert_eq!(runtime.share_id, GEMINI_CANVAS_DEFAULT_SHARE_ID);
        assert_eq!(runtime.api_base_url, GEMINI_CANVAS_DEFAULT_API_BASE_URL);
    }

    #[test]
    fn pure_http_mode_reads_preferred_required_and_disabled() {
        assert_eq!(
            pure_http_mode(&pure_http_mode_payload(Some("preferred"))),
            GeminiCanvasPureHttpMode::Preferred
        );
        assert_eq!(
            pure_http_mode(&pure_http_mode_payload(Some("required"))),
            GeminiCanvasPureHttpMode::Required
        );
        assert_eq!(
            pure_http_mode(&pure_http_mode_payload(Some("disabled"))),
            GeminiCanvasPureHttpMode::Disabled
        );
    }

    #[test]
    fn pure_http_required_only_accepts_required_mode() {
        assert!(!pure_http_required(&pure_http_mode_payload(Some(
            "preferred"
        ))));
        assert!(pure_http_required(&pure_http_mode_payload(Some(
            "required"
        ))));
        assert!(!pure_http_required(&pure_http_mode_payload(Some(
            "disabled"
        ))));
    }

    #[test]
    fn browser_runtime_state_object_key_prefers_explicit_browser_override() {
        let mut payload = pure_http_mode_payload(None);
        payload.runtime_state_object_key =
            Some("credential-runtime/gemini-canvas/storage-state.json".to_string());
        payload.extra_body = Some(HashMap::from([(
            "browserRuntimeStateObjectKey".to_string(),
            Value::String("credential-runtime/gemini-canvas/browser/profile".to_string()),
        )]));

        assert_eq!(
            browser_runtime_state_object_key(&payload).as_deref(),
            Some("credential-runtime/gemini-canvas/browser/profile")
        );
    }

    #[test]
    fn browser_runtime_state_object_key_falls_back_to_primary_runtime_state() {
        let mut payload = pure_http_mode_payload(None);
        payload.runtime_state_object_key =
            Some("credential-runtime/gemini-canvas/storage-state.json".to_string());
        payload.extra_body = Some(HashMap::new());

        assert_eq!(
            browser_runtime_state_object_key(&payload).as_deref(),
            Some("credential-runtime/gemini-canvas/storage-state.json")
        );
    }

    #[test]
    fn browser_cdp_url_reads_explicit_extra_body_value() {
        let mut payload = pure_http_mode_payload(None);
        payload.extra_body = Some(HashMap::from([(
            "browserCdpUrl".to_string(),
            Value::String("http://127.0.0.1:9334".to_string()),
        )]));

        assert_eq!(
            browser_cdp_url(&payload).as_deref(),
            Some("http://127.0.0.1:9334")
        );
    }

    #[test]
    fn browser_cookie_header_prefers_extra_body_cookie_value() {
        let mut payload = pure_http_mode_payload(None);
        payload
            .headers
            .insert("Cookie".to_string(), "stale=1".to_string());
        payload.extra_body = Some(HashMap::from([(
            "cookieHeader".to_string(),
            Value::String("fresh=1; __Secure-1PSID=abc".to_string()),
        )]));

        assert_eq!(
            browser_cookie_header(&payload).as_deref(),
            Some("fresh=1; __Secure-1PSID=abc")
        );
    }

    #[test]
    fn text_prompt_renders_system_and_user_transcript() {
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: Some(GEMINI_CANVAS_DEFAULT_TEXT_MODEL.to_string()),
            stream: false,
            messages: vec![
                CanonicalMessage {
                    role: MessageRole::System,
                    content: vec![ContentPart::Text {
                        text: "You are terse.".to_string(),
                    }],
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![],
                },
                CanonicalMessage {
                    role: MessageRole::User,
                    content: vec![ContentPart::Text {
                        text: "Reply with exactly: ok".to_string(),
                    }],
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![],
                },
            ],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        };

        let prompt = prompt_for_text_request(&req, "missing", "missing_code").unwrap();
        assert!(prompt.contains("You are terse."));
        assert!(prompt.contains("User request:\nReply with exactly: ok"));
        assert!(prompt.contains("Return exactly this text and nothing else:\nok"));
    }

    #[test]
    fn text_prompt_injects_tool_xml_for_required_tool_requests() {
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: Some(GEMINI_CANVAS_DEFAULT_TEXT_MODEL.to_string()),
            stream: false,
            messages: vec![
                CanonicalMessage {
                    role: MessageRole::System,
                    content: vec![ContentPart::Text {
                        text: "You must call exactly one tool and do not answer directly."
                            .to_string(),
                    }],
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![],
                },
                CanonicalMessage {
                    role: MessageRole::User,
                    content: vec![ContentPart::Text {
                        text: "Use only the weather tool for Hangzhou.".to_string(),
                    }],
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![],
                },
            ],
            tools: vec![CanonicalTool {
                tool_type: "function".to_string(),
                name: Some("weather".to_string()),
                description: Some("Return the current weather for a city.".to_string()),
                input_schema: Some(json!({
                    "type": "object",
                    "properties": { "city": { "type": "string" } },
                    "required": ["city"],
                })),
                raw: HashMap::new(),
            }],
            tool_choice: Some(json!("required")),
            reasoning: None,
            metadata: None,
            raw_body: json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        };

        let prompt = prompt_for_text_request(&req, "missing", "missing_code").unwrap();
        assert!(prompt.contains("TOOL CALL FORMAT"));
        assert!(prompt.contains("<tool_calls>"));
        assert!(prompt.contains("weather"));
        assert!(prompt.contains("Use only the weather tool for Hangzhou."));
    }

    #[test]
    fn text_prompt_serializes_tool_history_for_roundtrip() {
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: Some(GEMINI_CANVAS_DEFAULT_TEXT_MODEL.to_string()),
            stream: false,
            messages: vec![
                CanonicalMessage {
                    role: MessageRole::User,
                    content: vec![ContentPart::Text {
                        text: "Use the weather tool for Hangzhou.".to_string(),
                    }],
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![],
                },
                CanonicalMessage {
                    role: MessageRole::Assistant,
                    content: vec![ContentPart::Text {
                        text: String::new(),
                    }],
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![CanonicalToolCall {
                        id: Some("call_weather".to_string()),
                        call_type: "function".to_string(),
                        name: Some("weather".to_string()),
                        arguments: Some("{\"city\":\"Hangzhou\"}".to_string()),
                        raw: HashMap::new(),
                    }],
                },
                CanonicalMessage {
                    role: MessageRole::Tool,
                    content: vec![ContentPart::Text {
                        text: "{\"city\":\"Hangzhou\",\"condition\":\"sunny\"}".to_string(),
                    }],
                    name: None,
                    tool_call_id: Some("call_weather".to_string()),
                    tool_calls: vec![],
                },
                CanonicalMessage {
                    role: MessageRole::User,
                    content: vec![ContentPart::Text {
                        text: "Produce the final answer now.".to_string(),
                    }],
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![],
                },
            ],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        };

        let prompt = prompt_for_text_request(&req, "missing", "missing_code").unwrap();
        assert!(prompt.contains("Caller-provided result for `weather`"));
        assert!(prompt.contains("Produce the final answer now."));
    }

    #[test]
    fn text_prompt_surfaces_exact_answer_requirement_for_tool_history() {
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: Some(GEMINI_CANVAS_DEFAULT_TEXT_MODEL.to_string()),
            stream: false,
            messages: vec![
                CanonicalMessage {
                    role: MessageRole::System,
                    content: vec![ContentPart::Text {
                        text: "You are terse. Reply with exactly: gemini roundtrip ok".to_string(),
                    }],
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![],
                },
                CanonicalMessage {
                    role: MessageRole::Assistant,
                    content: vec![],
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![crate::protocol::canonical::CanonicalToolCall {
                        id: Some("call_weather".to_string()),
                        call_type: "function".to_string(),
                        name: Some("weather".to_string()),
                        arguments: Some("{\"city\":\"Hangzhou\"}".to_string()),
                        raw: HashMap::new(),
                    }],
                },
                CanonicalMessage {
                    role: MessageRole::Tool,
                    content: vec![ContentPart::Text {
                        text: "{\"city\":\"Hangzhou\",\"condition\":\"sunny\"}".to_string(),
                    }],
                    name: None,
                    tool_call_id: Some("call_weather".to_string()),
                    tool_calls: vec![],
                },
                CanonicalMessage {
                    role: MessageRole::User,
                    content: vec![ContentPart::Text {
                        text: "Produce the final answer now.".to_string(),
                    }],
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![],
                },
            ],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: Some(HashMap::new()),
            raw_body: json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        };

        let prompt = prompt_for_text_request(&req, "missing", "missing_code").unwrap();
        assert!(prompt.contains("Required exact final answer:\ngemini roundtrip ok"));
        assert!(prompt.contains("Return exactly that text and nothing else."));
    }

    #[test]
    fn build_text_stream_generate_heavy_request_uses_expected_slots() {
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: Some(GEMINI_CANVAS_DEFAULT_TEXT_MODEL.to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "Reply with exactly: heavy ok".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        };
        let bootstrap = gemini_web::GeminiWebBootstrap {
            access_token: Some("at-1".to_string()),
            build_label: Some("bl-1".to_string()),
            session_id: Some("sid-1".to_string()),
            language: "zh-CN".to_string(),
            push_id: None,
            client_pctx: None,
            app_page_path: None,
        };
        let request_uuid = "CDCF4DAC-9B46-4912-9D99-D52134A006A5";

        let request =
            build_text_stream_generate_heavy_request(&req, &bootstrap, request_uuid).unwrap();
        assert!(request
            .query
            .iter()
            .any(|(key, value)| key == "bl" && value == "bl-1"));
        assert!(request
            .query
            .iter()
            .any(|(key, value)| key == "f.sid" && value == "sid-1"));
        assert!(!request.form.iter().any(|(key, _)| key == "at"));

        let f_req = request
            .form
            .iter()
            .find(|(key, _)| key == "f.req")
            .map(|(_, value)| value)
            .unwrap();
        let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
        let inner = serde_json::from_str::<Vec<Value>>(outer[1].as_str().unwrap()).unwrap();
        assert_eq!(inner.len(), 80);
        assert_eq!(
            inner[0]
                .as_array()
                .and_then(|items| items.first())
                .and_then(Value::as_str),
            Some("Reply with exactly: heavy ok")
        );
        assert_eq!(
            inner[1]
                .as_array()
                .and_then(|items| items.first())
                .and_then(Value::as_str),
            Some("zh-CN")
        );
        assert_eq!(
            inner[3].as_str(),
            Some(GEMINI_CANVAS_TEXT_STREAM_GENERATE_OPAQUE_STATE)
        );
        assert_eq!(
            inner[6]
                .as_array()
                .and_then(|items| items.first())
                .and_then(Value::as_i64),
            Some(0)
        );
        assert_eq!(
            inner[41]
                .as_array()
                .and_then(|items| items.first())
                .and_then(Value::as_i64),
            Some(2)
        );
        assert_eq!(
            inner[49].as_i64(),
            Some(GEMINI_CANVAS_TEXT_STREAM_GENERATE_TEXT_MODE_INDEX)
        );
        assert_eq!(inner[59].as_str(), Some(request_uuid));
        assert_eq!(inner[68].as_i64(), Some(1));
        assert_eq!(inner[79].as_i64(), Some(1));
        let request_hex = inner[4].as_str().unwrap();
        assert_eq!(request_hex.len(), 32);
        assert!(request_hex.chars().all(|ch| ch.is_ascii_hexdigit()));
    }

    #[test]
    fn build_text_stream_generate_request_from_template_reuses_envelope() {
        let storage_state = json!({
            "cookies": [],
            "origins": [],
            "textStreamGenerateTemplate": {
                "url": "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate?bl=bl-1&f.sid=sid-1&hl=zh-CN&_reqid=12345&rt=c",
                "headers": {
                    "origin": "https://gemini.google.com",
                    "referer": "https://gemini.google.com/",
                    "x-goog-ext-525001261-jspb": "[1,null,null,null,\"fbb127bbb056c959\",null,null,0,[4],null,null,1,null,null,1,null,\"HEADER-ID\"]",
                    "x-goog-ext-525005358-jspb": "[\"REQ-ID\",1]"
                },
                "postData": "f.req=%5Bnull%2C%22%5B%5B%5C%22template%20prompt%5C%22%2C0%2Cnull%2Cnull%2Cnull%2Cnull%2C0%5D%2C%5B%5C%22zh-CN%5C%22%5D%2C%5B%5C%22%5C%22%2C%5C%22%5C%22%2C%5C%22%5C%22%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5C%22%5C%22%5D%2C%5C%22opaque-state-1%5C%22%2C%5C%22deadbeefdeadbeefdeadbeefdeadbeef%5C%22%2Cnull%2C%5B0%5D%2C1%2Cnull%2Cnull%2C1%2C0%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5B%5B0%5D%5D%2C0%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C1%2Cnull%2Cnull%2C%5B4%5D%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5B2%5D%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C0%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5C%22REQ-ID%5C%22%2Cnull%2C%5B%5D%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C1%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C1%5D%22%5D&"
            }
        });

        let request = build_text_stream_generate_request_from_template(
            &storage_state,
            "Reply with exactly: templated ok",
        )
        .unwrap()
        .unwrap();

        assert_eq!(
            request.url,
            "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate"
        );
        assert_eq!(
            request
                .query
                .iter()
                .find(|(key, _)| key == "bl")
                .map(|(_, value)| value.as_str()),
            Some("bl-1")
        );
        assert_eq!(
            request
                .headers
                .get("x-goog-ext-525005358-jspb")
                .map(String::as_str),
            Some("[\"REQ-ID\",1]")
        );
        let f_req = request
            .form
            .iter()
            .find(|(key, _)| key == "f.req")
            .map(|(_, value)| value)
            .unwrap();
        let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
        let inner = serde_json::from_str::<Vec<Value>>(outer[1].as_str().unwrap()).unwrap();
        assert_eq!(
            inner[0]
                .as_array()
                .and_then(|items| items.first())
                .and_then(Value::as_str),
            Some("Reply with exactly: templated ok")
        );
        assert_eq!(inner[3].as_str(), Some("opaque-state-1"));
        assert_eq!(inner[59].as_str(), Some("REQ-ID"));
        assert!(inner[49].is_null());
    }

    #[test]
    fn build_image_stream_generate_request_from_template_reuses_envelope_and_refreshes_request_uuid(
    ) {
        let storage_state = json!({
            "cookies": [],
            "origins": [],
            "imageStreamGenerateTemplate": {
                "url": "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate?bl=bl-1&f.sid=sid-1&hl=zh-CN&_reqid=12345&rt=c",
                "headers": {
                    "referer": "https://gemini.google.com/",
                    "x-goog-ext-525001261-jspb": "[1,null,null,null,\"56fdd199312815e2\",null,null,0,[4],null,null,2,null,null,1,null,\"HEADER-ID\"]",
                    "x-goog-ext-525005358-jspb": "[\"REQ-ID\",1]",
                    "x-goog-ext-73010989-jspb": "[0]",
                    "x-goog-ext-73010990-jspb": "[0]",
                    "x-same-domain": "1"
                },
                "postData": "f.req=%5Bnull%2C%22%5B%5B%5C%22template%20image%20prompt%5C%22%2C0%2Cnull%2Cnull%2Cnull%2Cnull%2C0%5D%2C%5B%5C%22zh-CN%5C%22%5D%2C%5B%5C%22%5C%22%2C%5C%22%5C%22%2C%5C%22%5C%22%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5C%22%5C%22%5D%2C%5C%22opaque-image-state-1%5C%22%2C%5C%22deadbeefdeadbeefdeadbeefdeadbeef%5C%22%2Cnull%2C%5B0%5D%2C1%2Cnull%2Cnull%2C1%2C0%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5B%5B0%5D%5D%2C0%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C1%2Cnull%2Cnull%2C%5B4%5D%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C14%2Cnull%2Cnull%2Cnull%2C0%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5C%22REQ-ID%5C%22%2Cnull%2C%5B%5D%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C1%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C1%5D%22%5D&at=AT-TOKEN"
            }
        });

        let request = build_image_stream_generate_request_from_template(
            &storage_state,
            "Reply with exactly: templated image ok",
            "NEW-REQ-ID",
        )
        .unwrap()
        .unwrap();

        assert_eq!(
            request.url,
            "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate"
        );
        assert_eq!(
            request
                .headers
                .get("x-goog-ext-525005358-jspb")
                .map(String::as_str),
            Some("[\"NEW-REQ-ID\",1]")
        );
        assert!(request.raw_post_data.contains("f.req="));
        assert!(request.raw_post_data.contains("at=AT-TOKEN"));
        assert!(request
            .form
            .iter()
            .any(|(key, value)| key == "at" && value == "AT-TOKEN"));
        let f_req = request
            .form
            .iter()
            .find(|(key, _)| key == "f.req")
            .map(|(_, value)| value)
            .unwrap();
        let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
        let inner = serde_json::from_str::<Vec<Value>>(outer[1].as_str().unwrap()).unwrap();
        assert_eq!(
            inner[0]
                .as_array()
                .and_then(|items| items.first())
                .and_then(Value::as_str),
            Some("Reply with exactly: templated image ok")
        );
        assert_eq!(inner[3].as_str(), Some("opaque-image-state-1"));
        assert_eq!(inner[59].as_str(), Some("NEW-REQ-ID"));
    }

    #[test]
    fn build_image_stream_generate_request_from_template_keeps_original_request_uuid_when_empty() {
        let storage_state = json!({
            "cookies": [],
            "origins": [],
            "imageStreamGenerateTemplate": {
                "url": "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate?bl=bl-1&f.sid=sid-1&hl=zh-CN&_reqid=12345&rt=c",
                "headers": {
                    "x-goog-ext-525005358-jspb": "[\"REQ-ID\",1]"
                },
                "postData": "f.req=%5Bnull%2C%22%5B%5B%5C%22template%20image%20prompt%5C%22%2C0%2Cnull%2Cnull%2Cnull%2Cnull%2C0%5D%2C%5B%5C%22zh-CN%5C%22%5D%2C%5B%5C%22%5C%22%2C%5C%22%5C%22%2C%5C%22%5C%22%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5C%22%5C%22%5D%2C%5C%22opaque-image-state-1%5C%22%2C%5C%22deadbeefdeadbeefdeadbeefdeadbeef%5C%22%2Cnull%2C%5B0%5D%2C1%2Cnull%2Cnull%2C1%2C0%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5B%5B0%5D%5D%2C0%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C1%2Cnull%2Cnull%2C%5B4%5D%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C14%2Cnull%2Cnull%2Cnull%2C0%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5C%22REQ-ID%5C%22%2Cnull%2C%5B%5D%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C1%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C1%5D%22%5D&at=AT-TOKEN"
            }
        });

        let request = build_image_stream_generate_request_from_template(
            &storage_state,
            "Reply with exactly: templated image ok",
            "",
        )
        .unwrap()
        .unwrap();

        assert_eq!(
            request
                .headers
                .get("x-goog-ext-525005358-jspb")
                .map(String::as_str),
            Some("[\"REQ-ID\",1]")
        );
        let f_req = request
            .form
            .iter()
            .find(|(key, _)| key == "f.req")
            .map(|(_, value)| value)
            .unwrap();
        let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
        let inner = serde_json::from_str::<Vec<Value>>(outer[1].as_str().unwrap()).unwrap();
        assert_eq!(
            inner[0]
                .as_array()
                .and_then(|items| items.first())
                .and_then(Value::as_str),
            Some("Reply with exactly: templated image ok")
        );
    }

    #[test]
    fn refresh_stream_generate_template_with_bootstrap_replaces_query_and_at() {
        let template = GeminiCanvasTextStreamGenerateTemplate {
            url: "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate?bl=old-bl&f.sid=old-sid&hl=en-US&_reqid=12345&rt=c".to_string(),
            query: vec![
                ("bl".to_string(), "old-bl".to_string()),
                ("f.sid".to_string(), "old-sid".to_string()),
                ("hl".to_string(), "en-US".to_string()),
                ("_reqid".to_string(), "12345".to_string()),
                ("rt".to_string(), "c".to_string()),
            ],
            form: vec![
                ("f.req".to_string(), "[null,\"[]\"]".to_string()),
                ("at".to_string(), "old-at".to_string()),
            ],
            raw_post_data: "f.req=%5Bnull%2C%22%5B%5D%22%5D&at=old-at".to_string(),
            headers: HashMap::new(),
        };
        let bootstrap = gemini_web::GeminiWebBootstrap {
            access_token: Some("new-at".to_string()),
            build_label: Some("new-bl".to_string()),
            session_id: Some("new-sid".to_string()),
            language: "zh-CN".to_string(),
            push_id: None,
            client_pctx: None,
            app_page_path: None,
        };

        let refreshed =
            refresh_stream_generate_template_with_bootstrap(&template, &bootstrap, true).unwrap();
        assert_eq!(
            refreshed.url,
            "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate"
        );
        assert_eq!(
            refreshed
                .query
                .iter()
                .find(|(key, _)| key == "bl")
                .map(|(_, value)| value.as_str()),
            Some("new-bl")
        );
        assert_eq!(
            refreshed
                .query
                .iter()
                .find(|(key, _)| key == "f.sid")
                .map(|(_, value)| value.as_str()),
            Some("new-sid")
        );
        assert_eq!(
            refreshed
                .query
                .iter()
                .find(|(key, _)| key == "hl")
                .map(|(_, value)| value.as_str()),
            Some("zh-CN")
        );
        assert_eq!(
            refreshed
                .form
                .iter()
                .find(|(key, _)| key == "at")
                .map(|(_, value)| value.as_str()),
            Some("new-at")
        );
        assert!(refreshed.raw_post_data.contains("at=new-at"));
    }

    #[test]
    fn refresh_stream_generate_template_access_token_replaces_only_at() {
        let template = GeminiCanvasTextStreamGenerateTemplate {
            url: "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate".to_string(),
            query: vec![
                ("bl".to_string(), "old-bl".to_string()),
                ("f.sid".to_string(), "old-sid".to_string()),
            ],
            form: vec![
                ("f.req".to_string(), "[null,\"[]\"]".to_string()),
                ("at".to_string(), "old-at".to_string()),
            ],
            raw_post_data: "f.req=%5Bnull%2C%22%5B%5D%22%5D&at=old-at".to_string(),
            headers: HashMap::from([("accept".to_string(), "*/*".to_string())]),
        };

        let refreshed = refresh_stream_generate_template_access_token(&template, "new-at");
        assert_eq!(refreshed.url, template.url);
        assert_eq!(refreshed.query, template.query);
        assert_eq!(
            refreshed
                .form
                .iter()
                .find(|(key, _)| key == "at")
                .map(|(_, value)| value.as_str()),
            Some("new-at")
        );
        assert!(refreshed.raw_post_data.contains("at=new-at"));
        assert_eq!(refreshed.headers, template.headers);
    }

    #[test]
    fn refresh_stream_generate_template_model_header_id_rewrites_suffix_uuid() {
        let mut template = GeminiCanvasTextStreamGenerateTemplate {
            url: "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate".to_string(),
            query: vec![],
            form: vec![],
            raw_post_data: "f.req=[]".to_string(),
            headers: HashMap::from([(
                "x-goog-ext-525001261-jspb".to_string(),
                "[1,null,null,null,\"56fdd199312815e2\",null,null,0,[4],null,null,2,null,null,1,null,\"OLD-HEADER-ID\"]".to_string(),
            )]),
        };

        assert!(refresh_stream_generate_template_model_header_id(
            &mut template,
            "NEW-HEADER-ID"
        ));

        let parsed = serde_json::from_str::<Vec<Value>>(
            template
                .headers
                .get("x-goog-ext-525001261-jspb")
                .expect("model header"),
        )
        .expect("parsed model header");
        assert_eq!(parsed[16].as_str(), Some("NEW-HEADER-ID"));
    }

    #[test]
    fn refresh_stream_generate_template_request_hex_rewrites_inner_slot() {
        let mut template = GeminiCanvasTextStreamGenerateTemplate {
            url: "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate".to_string(),
            query: vec![],
            form: vec![(
                "f.req".to_string(),
                "[null,\"[[\\\"prompt\\\",0,null,null,null,null,0],[\\\"zh-CN\\\"],[\\\"\\\",\\\"\\\",\\\"\\\",null,null,null,null,null,null,\\\"\\\"],\\\"opaque\\\",\\\"0123456789abcdef0123456789abcdef\\\",null,[1],1]\"]".to_string(),
            )],
            raw_post_data: "f.req=%5Bnull%2C%22%5B%5B%5C%22prompt%5C%22%2C0%2Cnull%2Cnull%2Cnull%2Cnull%2C0%5D%2C%5B%5C%22zh-CN%5C%22%5D%2C%5B%5C%22%5C%22%2C%5C%22%5C%22%2C%5C%22%5C%22%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5C%22%5C%22%5D%2C%5C%22opaque%5C%22%2C%5C%220123456789abcdef0123456789abcdef%5C%22%2Cnull%2C%5B1%5D%2C1%5D%22%5D".to_string(),
            headers: HashMap::new(),
        };

        assert!(refresh_stream_generate_template_request_hex(
            &mut template,
            "fedcba9876543210fedcba9876543210"
        ));

        let f_req = template
            .form
            .iter()
            .find(|(key, _)| key == "f.req")
            .map(|(_, value)| value)
            .expect("f.req");
        let outer = serde_json::from_str::<Vec<Value>>(f_req).expect("outer");
        let inner = serde_json::from_str::<Vec<Value>>(outer[1].as_str().expect("inner json"))
            .expect("inner");
        assert_eq!(inner[4].as_str(), Some("fedcba9876543210fedcba9876543210"));
        assert!(template
            .raw_post_data
            .contains("fedcba9876543210fedcba9876543210"));
    }

    #[test]
    fn build_stream_generate_heavy_request_sets_requested_mode_index() {
        let _req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: Some(GEMINI_CANVAS_DEFAULT_TEXT_MODEL.to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "Reply with exactly: media ok".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        };
        let bootstrap = gemini_web::GeminiWebBootstrap {
            access_token: Some("at-1".to_string()),
            build_label: Some("bl-1".to_string()),
            session_id: Some("sid-1".to_string()),
            language: "en-US".to_string(),
            push_id: None,
            client_pctx: None,
            app_page_path: None,
        };

        let request = build_stream_generate_heavy_request(
            "Reply with exactly: media ok",
            &bootstrap,
            "CDCF4DAC-9B46-4912-9D99-D52134A006A5",
            GEMINI_CANVAS_STREAM_GENERATE_VIDEO_MODE_INDEX,
        )
        .unwrap();
        let f_req = request
            .form
            .iter()
            .find(|(key, _)| key == "f.req")
            .map(|(_, value)| value)
            .unwrap();
        let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
        let inner = serde_json::from_str::<Vec<Value>>(outer[1].as_str().unwrap()).unwrap();
        assert_eq!(
            inner[49].as_i64(),
            Some(GEMINI_CANVAS_STREAM_GENERATE_VIDEO_MODE_INDEX)
        );
        assert_eq!(
            inner[3].as_str(),
            Some(GEMINI_CANVAS_MEDIA_STREAM_GENERATE_OPAQUE_STATE)
        );
        assert_eq!(
            inner[6]
                .as_array()
                .and_then(|items| items.first())
                .and_then(Value::as_i64),
            Some(1)
        );
        assert_eq!(inner[68].as_i64(), Some(2));
    }

    #[test]
    fn build_stream_generate_image_request_uses_image_mode_shape() {
        let bootstrap = gemini_web::GeminiWebBootstrap {
            access_token: Some("at-1".to_string()),
            build_label: Some("bl-1".to_string()),
            session_id: Some("sid-1".to_string()),
            language: "zh-CN".to_string(),
            push_id: None,
            client_pctx: None,
            app_page_path: None,
        };

        let request = build_stream_generate_heavy_request(
            "Reply with exactly: image mode ok",
            &bootstrap,
            "3FCBC5C2-4C8B-468E-B715-B5392A5AED35",
            GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX,
        )
        .unwrap();
        let f_req = request
            .form
            .iter()
            .find(|(key, _)| key == "f.req")
            .map(|(_, value)| value)
            .unwrap();
        let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
        let inner = serde_json::from_str::<Vec<Value>>(outer[1].as_str().unwrap()).unwrap();
        assert_eq!(
            inner[6]
                .as_array()
                .and_then(|items| items.first())
                .and_then(Value::as_i64),
            Some(0)
        );
        assert_eq!(
            inner[49].as_i64(),
            Some(GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX)
        );
        assert_eq!(
            inner[3].as_str(),
            Some(GEMINI_CANVAS_IMAGE_STREAM_GENERATE_OPAQUE_STATE)
        );
        assert_eq!(inner[68].as_i64(), Some(1));
        assert_eq!(inner[79].as_i64(), Some(1));
    }

    #[test]
    fn build_stream_generate_image_edit_request_includes_uploaded_refs() {
        let bootstrap = gemini_web::GeminiWebBootstrap {
            access_token: Some("at-1".to_string()),
            build_label: Some("bl-1".to_string()),
            session_id: Some("sid-1".to_string()),
            language: "zh-CN".to_string(),
            push_id: Some("push-1".to_string()),
            client_pctx: Some("pctx-1".to_string()),
            app_page_path: None,
        };

        let request = build_stream_generate_heavy_request_with_uploaded_files(
            "把上传的样例图编辑成一个霓虹徽章",
            &bootstrap,
            "3FCBC5C2-4C8B-468E-B715-B5392A5AED35",
            GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX,
            &[GeminiCanvasUploadedFileRef {
                resource_path: "/contrib_service/ttl_1d/example".to_string(),
                mime_type: "image/jpeg".to_string(),
                file_name: "edit-source.jpg".to_string(),
            }],
        )
        .unwrap();
        let f_req = request
            .form
            .iter()
            .find(|(key, _)| key == "f.req")
            .map(|(_, value)| value)
            .unwrap();
        let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
        let inner = serde_json::from_str::<Vec<Value>>(outer[1].as_str().unwrap()).unwrap();
        assert_eq!(
            inner[0][3][0][0][0].as_str(),
            Some("/contrib_service/ttl_1d/example")
        );
        assert_eq!(inner[0][3][0][0][1].as_i64(), Some(1));
        assert_eq!(inner[0][3][0][0][3].as_str(), Some("image/jpeg"));
        assert_eq!(inner[0][3][0][1].as_str(), Some("edit-source.jpg"));
        assert_eq!(inner[0][3][0][8][0].as_i64(), Some(0));
        assert_eq!(inner[6][0].as_i64(), Some(1));
        assert_eq!(inner[68].as_i64(), Some(2));
    }

    #[test]
    fn build_stream_generate_image_edit_request_uses_seeded_state_when_provided() {
        let bootstrap = gemini_web::GeminiWebBootstrap {
            access_token: Some("at-1".to_string()),
            build_label: Some("bl-1".to_string()),
            session_id: Some("sid-1".to_string()),
            language: "zh-CN".to_string(),
            push_id: Some("push-1".to_string()),
            client_pctx: Some("pctx-1".to_string()),
            app_page_path: None,
        };
        let seed = GeminiCanvasStreamGenerateSeed {
            opaque_state: Some("seed-opaque-state".to_string()),
            request_hex: Some("0123456789abcdef0123456789abcdef".to_string()),
            request_uuid: Some("seed-request-id".to_string()),
        };

        let request = build_stream_generate_heavy_request_with_uploaded_files_seeded(
            "把上传的样例图编辑成一个霓虹徽章",
            &bootstrap,
            "seed-request-id",
            GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX,
            &[GeminiCanvasUploadedFileRef {
                resource_path: "/contrib_service/ttl_1d/example".to_string(),
                mime_type: "image/jpeg".to_string(),
                file_name: "edit-source.jpg".to_string(),
            }],
            Some(&seed),
        )
        .unwrap();
        let f_req = request
            .form
            .iter()
            .find(|(key, _)| key == "f.req")
            .map(|(_, value)| value)
            .unwrap();
        let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
        let inner = serde_json::from_str::<Vec<Value>>(outer[1].as_str().unwrap()).unwrap();
        assert_eq!(inner[3].as_str(), Some("seed-opaque-state"));
        assert_eq!(inner[4].as_str(), Some("0123456789abcdef0123456789abcdef"));
        assert_eq!(inner[59].as_str(), Some("seed-request-id"));
        assert_eq!(inner[6][0].as_i64(), Some(1));
        assert_eq!(inner[68].as_i64(), Some(2));
    }

    #[test]
    fn harvest_image_edit_stream_generate_seed_reads_template_slots() {
        let storage_state = json!({
            "imageEditStreamGenerateTemplate": {
                "postData": "f.req=%5Bnull%2C%22%5B%5B%5C%22template%20image%20prompt%5C%22%2C0%2Cnull%2C%5B%5B%5B%5C%22%2Fcontrib_service%2Fttl_1d%2Fexample%5C%22%2C1%2Cnull%2C%5C%22image%2Fjpeg%5C%22%5D%2C%5C%22edit-source.jpg%5C%22%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5B0%5D%5D%5D%2Cnull%2Cnull%2C0%5D%2C%5B%5C%22zh-CN%5C%22%5D%2C%5B%5C%22%5C%22%2C%5C%22%5C%22%2C%5C%22%5C%22%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5C%22%5C%22%5D%2C%5C%22seed-opaque-state%5C%22%2C%5C%220123456789abcdef0123456789abcdef%5C%22%2Cnull%2C%5B1%5D%2C1%2Cnull%2Cnull%2C1%2C0%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5B%5B0%5D%5D%2C0%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C1%2Cnull%2Cnull%2C%5B4%5D%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5B1%5D%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C14%2Cnull%2Cnull%2Cnull%2C0%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5C%22seed-request-id%5C%22%2Cnull%2C%5B%5D%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C2%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C1%5D%22%5D&at=AT-TOKEN"
            }
        });

        let seed = harvest_image_edit_stream_generate_seed(&storage_state).unwrap();
        assert_eq!(seed.opaque_state.as_deref(), Some("seed-opaque-state"));
        assert_eq!(
            seed.request_hex.as_deref(),
            Some("0123456789abcdef0123456789abcdef")
        );
        assert_eq!(seed.request_uuid.as_deref(), Some("seed-request-id"));
    }

    #[test]
    fn extract_image_edit_uploads_normalizes_png_inputs_to_jpeg_contract() {
        let mut png_bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png_bytes)
            .write_image(&[0, 0, 0, 255], 1, 1, image::ExtendedColorType::Rgba8)
            .unwrap();
        let png_base64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ImagesEdits,
            requested_model: Some(GEMINI_25_FLASH_IMAGE_PREVIEW_MODEL.to_string()),
            stream: false,
            messages: vec![],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({
                "prompt": "edit the sample",
                "images": [{
                    "mime_type": "image/png",
                    "base64": png_base64
                }],
                "response_format": "url"
            }),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        };

        let uploads = extract_image_edit_uploads(&req).unwrap();
        assert_eq!(uploads.len(), 1);
        assert_eq!(uploads[0].mime_type, "image/jpeg");
        assert_eq!(uploads[0].file_name, "edit-source.jpg");
        assert!(uploads[0].bytes.starts_with(&[0xFF, 0xD8, 0xFF]));
    }

    #[test]
    fn normalize_image_edit_upload_to_jpeg_caps_large_png_under_browser_budget() {
        let width = 1400_u32;
        let height = 1400_u32;
        let mut pixels = Vec::with_capacity((width * height * 3) as usize);
        for y in 0..height {
            for x in 0..width {
                pixels.push(((x * 13 + y * 3) % 256) as u8);
                pixels.push(((x * 7 + y * 11) % 256) as u8);
                pixels.push(((x * 5 + y * 17) % 256) as u8);
            }
        }
        let mut png_bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png_bytes)
            .write_image(&pixels, width, height, image::ExtendedColorType::Rgb8)
            .unwrap();

        let jpeg_bytes = normalize_image_edit_upload_to_jpeg("image/png", &png_bytes).unwrap();
        assert!(jpeg_bytes.starts_with(&[0xFF, 0xD8, 0xFF]));
        assert!(
            jpeg_bytes.len() <= 127_600,
            "expected browser-budget jpeg, got {} bytes",
            jpeg_bytes.len()
        );
        assert!(
            jpeg_bytes.len() >= 124_000,
            "expected search to stay near browser-budget jpeg, got {} bytes",
            jpeg_bytes.len()
        );
    }

    #[test]
    fn build_stream_generate_text_request_uses_text_mode_shape() {
        let bootstrap = gemini_web::GeminiWebBootstrap {
            access_token: Some("at-1".to_string()),
            build_label: Some("bl-1".to_string()),
            session_id: Some("sid-1".to_string()),
            language: "en-US".to_string(),
            push_id: None,
            client_pctx: None,
            app_page_path: None,
        };

        let request = build_stream_generate_heavy_request(
            "Reply with exactly: pure http smoke ok",
            &bootstrap,
            "CDCF4DAC-9B46-4912-9D99-D52134A006A5",
            GEMINI_CANVAS_TEXT_STREAM_GENERATE_TEXT_MODE_INDEX,
        )
        .unwrap();
        let f_req = request
            .form
            .iter()
            .find(|(key, _)| key == "f.req")
            .map(|(_, value)| value)
            .unwrap();
        let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
        let inner = serde_json::from_str::<Vec<Value>>(outer[1].as_str().unwrap()).unwrap();
        assert_eq!(
            inner[6]
                .as_array()
                .and_then(|items| items.first())
                .and_then(Value::as_i64),
            Some(0)
        );
        assert_eq!(
            inner[3].as_str(),
            Some(GEMINI_CANVAS_TEXT_STREAM_GENERATE_OPAQUE_STATE)
        );
        assert_eq!(
            inner[41]
                .as_array()
                .and_then(|items| items.first())
                .and_then(Value::as_i64),
            Some(2)
        );
        assert_eq!(
            inner[49].as_i64(),
            Some(GEMINI_CANVAS_TEXT_STREAM_GENERATE_TEXT_MODE_INDEX)
        );
        assert_eq!(inner[68].as_i64(), Some(1));
        assert_eq!(inner[79].as_i64(), Some(1));
    }

    #[test]
    fn text_prompt_preserves_system_instruction_for_plain_text_requests() {
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: Some(GEMINI_CANVAS_DEFAULT_TEXT_MODEL.to_string()),
            stream: false,
            messages: vec![
                CanonicalMessage {
                    role: MessageRole::System,
                    content: vec![ContentPart::Text {
                        text: "You are terse.".to_string(),
                    }],
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![],
                },
                CanonicalMessage {
                    role: MessageRole::User,
                    content: vec![ContentPart::Text {
                        text: "Reply with exactly: plain text ok".to_string(),
                    }],
                    name: None,
                    tool_call_id: None,
                    tool_calls: vec![],
                },
            ],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        };

        let prompt = prompt_for_text_request(&req, "missing", "missing_code").unwrap();
        assert!(prompt.contains("You are terse."));
        assert!(prompt.contains("User request:\nReply with exactly: plain text ok"));
        assert!(prompt.contains("Return exactly this text and nothing else:\nplain text ok"));
    }

    #[test]
    fn build_text_mode_selection_preflight_request_targets_last_selected_mode() {
        let bootstrap = gemini_web::GeminiWebBootstrap {
            access_token: None,
            build_label: Some("bl-1".to_string()),
            session_id: Some("sid-1".to_string()),
            language: "zh-CN".to_string(),
            push_id: None,
            client_pctx: None,
            app_page_path: None,
        };

        let request =
            build_text_mode_selection_preflight_request(&bootstrap, "/share/fe24c455a570").unwrap();
        assert!(request.query.iter().any(|(key, value)| {
            key == "rpcids" && value == GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID
        }));
        assert!(request
            .query
            .iter()
            .any(|(key, value)| key == "source-path" && value == "/share/fe24c455a570"));

        let f_req = request
            .form
            .iter()
            .find(|(key, _)| key == "f.req")
            .map(|(_, value)| value)
            .unwrap();
        let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
        let rpc = outer[0]
            .as_array()
            .and_then(|group| group.first())
            .and_then(Value::as_array)
            .unwrap();
        assert_eq!(
            rpc[0].as_str(),
            Some(GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID)
        );
        let payload = serde_json::from_str::<Vec<Value>>(rpc[1].as_str().unwrap()).unwrap();
        let state = payload[0].as_array().unwrap();
        assert_eq!(state.len(), 100);
        assert_eq!(
            state[99].as_str(),
            Some(GEMINI_CANVAS_TEXT_LAST_SELECTED_MODE_ID)
        );
        let keys = payload[1].as_array().unwrap();
        let key_group = keys[0].as_array().unwrap();
        assert_eq!(key_group[0].as_str(), Some("last_selected_mode_id_on_web"));
    }

    #[test]
    fn build_mode_selection_preflight_request_allows_media_selected_id() {
        let bootstrap = gemini_web::GeminiWebBootstrap {
            access_token: None,
            build_label: Some("bl-1".to_string()),
            session_id: Some("sid-1".to_string()),
            language: "zh-CN".to_string(),
            push_id: None,
            client_pctx: None,
            app_page_path: None,
        };

        let request = build_mode_selection_preflight_request(
            &bootstrap,
            "/app/abc123",
            GEMINI_CANVAS_TEXT_SELECTED_MODEL_HEADER_ID,
        )
        .unwrap();
        assert!(request.query.iter().any(|(key, value)| {
            key == "rpcids" && value == GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID
        }));
        assert!(request
            .query
            .iter()
            .any(|(key, value)| key == "source-path" && value == "/app/abc123"));

        let f_req = request
            .form
            .iter()
            .find(|(key, _)| key == "f.req")
            .map(|(_, value)| value)
            .unwrap();
        let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
        let rpc = outer[0]
            .as_array()
            .and_then(|group| group.first())
            .and_then(Value::as_array)
            .unwrap();
        let payload = serde_json::from_str::<Vec<Value>>(rpc[1].as_str().unwrap()).unwrap();
        let state = payload[0].as_array().unwrap();
        assert_eq!(state.len(), 100);
        assert_eq!(
            state[99].as_str(),
            Some(GEMINI_CANVAS_TEXT_SELECTED_MODEL_HEADER_ID)
        );
    }

    #[test]
    fn build_media_operation_selection_preflight_request_matches_captured_shape() {
        let bootstrap = gemini_web::GeminiWebBootstrap {
            access_token: None,
            build_label: Some("bl-1".to_string()),
            session_id: Some("sid-1".to_string()),
            language: "zh-CN".to_string(),
            push_id: None,
            client_pctx: None,
            app_page_path: None,
        };

        let request =
            build_media_operation_selection_preflight_request(&bootstrap, "/app/abc123", 11)
                .unwrap();
        assert!(request.query.iter().any(|(key, value)| {
            key == "rpcids" && value == GEMINI_CANVAS_MEDIA_OPERATION_SELECTION_RPCID
        }));
        assert!(request
            .query
            .iter()
            .any(|(key, value)| key == "source-path" && value == "/app/abc123"));

        let f_req = request
            .form
            .iter()
            .find(|(key, _)| key == "f.req")
            .map(|(_, value)| value)
            .unwrap();
        let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
        let rpc = outer[0]
            .as_array()
            .and_then(|group| group.first())
            .and_then(Value::as_array)
            .unwrap();
        assert_eq!(
            rpc[0].as_str(),
            Some(GEMINI_CANVAS_MEDIA_OPERATION_SELECTION_RPCID)
        );
        let payload = serde_json::from_str::<Vec<Value>>(rpc[1].as_str().unwrap()).unwrap();
        assert_eq!(Value::Array(payload), json!([[[1, 11], [2, 11], [6, 11]]]));
    }

    #[test]
    fn build_media_operation_selection_preflight_request_maps_image_mode_to_captured_index() {
        let bootstrap = gemini_web::GeminiWebBootstrap {
            access_token: None,
            build_label: Some("bl-1".to_string()),
            session_id: Some("sid-1".to_string()),
            language: "zh-CN".to_string(),
            push_id: None,
            client_pctx: None,
            app_page_path: None,
        };

        let request = build_media_operation_selection_preflight_request(
            &bootstrap,
            "/app",
            GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX,
        )
        .unwrap();
        let f_req = request
            .form
            .iter()
            .find(|(key, _)| key == "f.req")
            .map(|(_, value)| value)
            .unwrap();
        let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
        let rpc = outer[0]
            .as_array()
            .and_then(|group| group.first())
            .and_then(Value::as_array)
            .unwrap();
        let payload = serde_json::from_str::<Vec<Value>>(rpc[1].as_str().unwrap()).unwrap();
        assert_eq!(Value::Array(payload), json!([[[1, 11], [2, 11], [6, 11]]]));
    }

    #[test]
    fn build_text_bootstrap_preflight_request_uses_empty_payload() {
        let bootstrap = gemini_web::GeminiWebBootstrap {
            access_token: Some("AT".to_string()),
            build_label: Some("bl-1".to_string()),
            session_id: Some("sid-1".to_string()),
            language: "zh-CN".to_string(),
            push_id: None,
            client_pctx: None,
            app_page_path: None,
        };

        let request =
            build_text_bootstrap_preflight_request(&bootstrap, "/share/fe24c455a570").unwrap();
        assert!(request.query.iter().any(|(key, value)| {
            key == "rpcids" && value == GEMINI_CANVAS_TEXT_BOOTSTRAP_RPCID
        }));
        assert!(request
            .query
            .iter()
            .any(|(key, value)| key == "source-path" && value == "/share/fe24c455a570"));

        let f_req = request
            .form
            .iter()
            .find(|(key, _)| key == "f.req")
            .map(|(_, value)| value)
            .unwrap();
        let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
        let rpc = outer[0]
            .as_array()
            .and_then(|group| group.first())
            .and_then(Value::as_array)
            .unwrap();
        assert_eq!(rpc[0].as_str(), Some(GEMINI_CANVAS_TEXT_BOOTSTRAP_RPCID));
        assert_eq!(rpc[1].as_str(), Some("[]"));
        assert_eq!(rpc[3].as_str(), Some("generic"));
        assert_eq!(
            request
                .form
                .iter()
                .find(|(key, _)| key == "at")
                .map(|(_, value)| value.as_str()),
            Some("AT")
        );
    }

    #[test]
    fn build_text_state_preflight_request_uses_canvas_state_payload() {
        let bootstrap = gemini_web::GeminiWebBootstrap {
            access_token: None,
            build_label: Some("bl-1".to_string()),
            session_id: Some("sid-1".to_string()),
            language: "zh-CN".to_string(),
            push_id: None,
            client_pctx: None,
            app_page_path: None,
        };

        let request = build_text_state_preflight_request(&bootstrap, "/app").unwrap();
        assert!(request.query.iter().any(|(key, value)| {
            key == "rpcids" && value == GEMINI_CANVAS_TEXT_STATE_PREFLIGHT_RPCID
        }));
        assert!(request
            .query
            .iter()
            .any(|(key, value)| key == "source-path" && value == "/app"));

        let f_req = request
            .form
            .iter()
            .find(|(key, _)| key == "f.req")
            .map(|(_, value)| value)
            .unwrap();
        let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
        let rpc = outer[0]
            .as_array()
            .and_then(|group| group.first())
            .and_then(Value::as_array)
            .unwrap();
        assert_eq!(
            rpc[0].as_str(),
            Some(GEMINI_CANVAS_TEXT_STATE_PREFLIGHT_RPCID)
        );
        assert_eq!(rpc[1].as_str(), Some("[[null,null,null,null,true]]"));
        assert_eq!(rpc[3].as_str(), Some("generic"));
    }

    #[test]
    fn build_tts_trigger_request_uses_response_id_payload() {
        let bootstrap = gemini_web::GeminiWebBootstrap {
            access_token: None,
            build_label: Some("bl-1".to_string()),
            session_id: Some("sid-1".to_string()),
            language: "zh-CN".to_string(),
            push_id: None,
            client_pctx: None,
            app_page_path: None,
        };

        let request = build_tts_trigger_request("r_e0e4aa76ab2755e3", &bootstrap, "/app").unwrap();
        assert!(request
            .query
            .iter()
            .any(|(key, value)| key == "rpcids" && value == GEMINI_CANVAS_TTS_TRIGGER_RPCID));
        assert!(request
            .query
            .iter()
            .any(|(key, value)| key == "source-path" && value == "/app"));

        let f_req = request
            .form
            .iter()
            .find(|(key, _)| key == "f.req")
            .map(|(_, value)| value)
            .unwrap();
        let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
        let rpc = outer[0]
            .as_array()
            .and_then(|group| group.first())
            .and_then(Value::as_array)
            .unwrap();
        assert_eq!(rpc[0].as_str(), Some(GEMINI_CANVAS_TTS_TRIGGER_RPCID));
        assert_eq!(rpc[1].as_str(), Some("[\"r_e0e4aa76ab2755e3\"]"));
        assert_eq!(rpc[3].as_str(), Some("generic"));
    }

    #[test]
    fn build_music_trigger_request_uses_response_id_payload() {
        let bootstrap = gemini_web::GeminiWebBootstrap {
            access_token: None,
            build_label: Some("bl-1".to_string()),
            session_id: Some("sid-1".to_string()),
            language: "zh-CN".to_string(),
            push_id: None,
            client_pctx: None,
            app_page_path: None,
        };

        let request =
            build_music_trigger_request("r_a7578d122722b7b5", &bootstrap, "/app").unwrap();
        assert!(request
            .query
            .iter()
            .any(|(key, value)| key == "rpcids" && value == GEMINI_CANVAS_TTS_TRIGGER_RPCID));
        assert!(request
            .query
            .iter()
            .any(|(key, value)| key == "source-path" && value == "/app"));

        let f_req = request
            .form
            .iter()
            .find(|(key, _)| key == "f.req")
            .map(|(_, value)| value)
            .unwrap();
        let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
        let rpc = outer[0]
            .as_array()
            .and_then(|group| group.first())
            .and_then(Value::as_array)
            .unwrap();
        assert_eq!(rpc[0].as_str(), Some(GEMINI_CANVAS_TTS_TRIGGER_RPCID));
        assert_eq!(rpc[1].as_str(), Some("[\"r_a7578d122722b7b5\"]"));
        assert_eq!(rpc[3].as_str(), Some("generic"));
    }

    #[test]
    fn infer_tts_export_locale_prefers_detected_english_language() {
        assert_eq!(
            infer_tts_export_locale("Hello from Gemini Canvas!", "zh-CN"),
            "en-CN"
        );
    }

    #[test]
    fn build_tts_audio_export_request_uses_text_and_locale_payload() {
        let bootstrap = gemini_web::GeminiWebBootstrap {
            access_token: None,
            build_label: Some("bl-1".to_string()),
            session_id: Some("sid-1".to_string()),
            language: "zh-CN".to_string(),
            push_id: None,
            client_pctx: None,
            app_page_path: None,
        };

        let request = build_tts_audio_export_request(
            "Hello from Gemini Canvas!",
            "en-CN",
            &bootstrap,
            "/app/85cba2bfe36a963e",
        )
        .unwrap();
        assert!(request
            .query
            .iter()
            .any(|(key, value)| key == "rpcids" && value == GEMINI_CANVAS_TTS_EXPORT_RPCID));
        assert!(request
            .query
            .iter()
            .any(|(key, value)| key == "source-path" && value == "/app/85cba2bfe36a963e"));

        let f_req = request
            .form
            .iter()
            .find(|(key, _)| key == "f.req")
            .map(|(_, value)| value)
            .unwrap();
        let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
        let rpc = outer[0]
            .as_array()
            .and_then(|group| group.first())
            .and_then(Value::as_array)
            .unwrap();
        assert_eq!(rpc[0].as_str(), Some(GEMINI_CANVAS_TTS_EXPORT_RPCID));
        let payload = serde_json::from_str::<Vec<Value>>(rpc[1].as_str().unwrap()).unwrap();
        assert!(payload[0].is_null());
        assert_eq!(payload[1].as_str(), Some("Hello from Gemini Canvas!"));
        assert_eq!(payload[2].as_str(), Some("en-CN"));
        assert_eq!(payload[4].as_i64(), Some(2));
        assert_eq!(rpc[3].as_str(), Some("generic"));
    }

    #[test]
    fn extract_audio_from_tts_export_response_decodes_ogg_payload() {
        let frame = serde_json::to_string(&vec![json!([
            "wrb.fr",
            "XqA3Ic",
            "[\"T2dnUw==\"]",
            null,
            null,
            null,
            "generic"
        ])])
        .unwrap();
        let body = format!(")]}}'\n\n{}\n{}\n", frame.len(), frame);
        let audio = extract_audio_from_tts_export_response(&body).unwrap();
        assert_eq!(audio.mime_type, "audio/ogg");
        assert_eq!(audio.bytes, b"OggS");
    }

    #[test]
    fn extract_audio_from_tts_export_response_scans_raw_body_when_frame_length_is_invalid() {
        let frame = serde_json::to_string(&vec![json!([
            "wrb.fr",
            "XqA3Ic",
            "[\"T2dnUw\"]",
            null,
            null,
            null,
            "generic"
        ])])
        .unwrap();
        let body = format!(")]}}'\n\n999999\n{}\n", frame);
        let audio = extract_audio_from_tts_export_response(&body).unwrap();
        assert_eq!(audio.mime_type, "audio/ogg");
        assert_eq!(audio.bytes, b"OggS");
    }

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

    #[test]
    fn build_image_state_keys_preflight_request_matches_captured_shape() {
        let bootstrap = gemini_web::GeminiWebBootstrap {
            access_token: Some("AT".to_string()),
            build_label: Some("boq".to_string()),
            session_id: Some("123".to_string()),
            language: "zh-CN".to_string(),
            push_id: None,
            client_pctx: None,
            app_page_path: None,
        };

        let request = build_image_state_keys_preflight_request(&bootstrap, "/app").unwrap();
        let f_req = request
            .form
            .iter()
            .find(|(key, _)| key == "f.req")
            .map(|(_, value)| value)
            .unwrap();
        let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
        let rpc = outer[0]
            .as_array()
            .and_then(|group| group.first())
            .and_then(Value::as_array)
            .unwrap();
        assert_eq!(
            rpc[0].as_str(),
            Some(GEMINI_CANVAS_TEXT_STATE_PREFLIGHT_RPCID)
        );
        let payload = serde_json::from_str::<Value>(rpc[1].as_str().unwrap()).unwrap();
        assert_eq!(
            payload.pointer("/0/0/0").and_then(Value::as_str),
            Some("adaptive_device_responses_enabled")
        );
        assert_eq!(
            payload
                .pointer(
                    format!(
                        "/0/0/{}",
                        GEMINI_CANVAS_IMAGE_STATE_KEYS_PREFLIGHT_KEYS.len() - 1
                    )
                    .as_str()
                )
                .and_then(Value::as_str),
            Some("zs_student_aip_banner_dismissal_count")
        );
    }

    #[test]
    fn build_conversation_list_probe_request_matches_capture_payload() {
        let bootstrap = gemini_web::GeminiWebBootstrap {
            access_token: Some("AT".to_string()),
            build_label: Some("boq".to_string()),
            session_id: Some("123".to_string()),
            language: "zh-CN".to_string(),
            push_id: None,
            client_pctx: None,
            app_page_path: None,
        };

        let request = build_conversation_list_probe_request(&bootstrap, "/app").unwrap();
        assert!(request
            .query
            .iter()
            .any(|(key, value)| key == "rpcids" && value == "MaZiqc"));
        assert!(request
            .query
            .iter()
            .any(|(key, value)| key == "source-path" && value == "/app"));
        let f_req = request
            .form
            .iter()
            .find(|(key, _)| key == "f.req")
            .map(|(_, value)| value)
            .unwrap();
        assert_eq!(
            f_req,
            "[[[\"MaZiqc\",\"[13,null,[1,null,1]]\",null,\"generic\"]]]"
        );
    }

    #[test]
    fn build_conversation_list_full_request_matches_capture_payload() {
        let bootstrap = gemini_web::GeminiWebBootstrap {
            access_token: Some("AT".to_string()),
            build_label: Some("boq".to_string()),
            session_id: Some("123".to_string()),
            language: "zh-CN".to_string(),
            push_id: None,
            client_pctx: None,
            app_page_path: None,
        };

        let request = build_conversation_list_full_request(&bootstrap, "/app").unwrap();
        assert!(request
            .query
            .iter()
            .any(|(key, value)| key == "rpcids" && value == "MaZiqc"));
        assert!(request
            .query
            .iter()
            .any(|(key, value)| key == "source-path" && value == "/app"));
        let f_req = request
            .form
            .iter()
            .find(|(key, _)| key == "f.req")
            .map(|(_, value)| value)
            .unwrap();
        assert_eq!(
            f_req,
            "[[[\"MaZiqc\",\"[13,null,[0,null,1]]\",null,\"generic\"]]]"
        );
    }

    #[test]
    fn extract_conversation_list_entries_reads_ids_titles_and_timestamps() {
        let body = concat!(
            ")]}'\n\n",
            "2131\n",
            "[[\"wrb.fr\",\"MaZiqc\",\"[null,\\\"token\\\",[[\\\"c_da24e84ec005599a\\\",\\\"canvas live skyline image\\\\n\\\\nRequested aspect ratio: 1:1.\\\",null,null,null,[1777601930,749541000],null,null,null,2],[\\\"c_90ca192406be80bb\\\",\\\"Canvas Live Launch Video Generation\\\",null,null,null,[1777501548,491550000],[[\\\"c_90ca192406be80bb\\\",\\\"r_bc98137b61ada92c\\\"],1],null,null,2]]]\",null,null,null,\"generic\"]]\n",
            "56\n",
            "[[\"di\",121],[\"af.httprm\",120,\"1294558254015590662\",5]]\n",
            "26\n",
            "[[\"e\",4,null,null,2231]]\n"
        );

        let entries = extract_conversation_list_entries(body);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].conversation_id, "c_da24e84ec005599a");
        assert_eq!(
            entries[0].title,
            "canvas live skyline image\n\nRequested aspect ratio: 1:1."
        );
        assert_eq!(entries[0].updated_at_secs, 1777601930);
        assert_eq!(entries[0].updated_at_nanos, 749541000);
        assert_eq!(entries[0].response_id, None);
        assert_eq!(
            entries[0].app_path().as_deref(),
            Some("/app/da24e84ec005599a")
        );
        assert_eq!(entries[1].conversation_id, "c_90ca192406be80bb");
        assert_eq!(
            entries[1].response_id.as_deref(),
            Some("r_bc98137b61ada92c")
        );
    }

    #[test]
    fn extract_stream_generate_locator_reads_response_and_conversation_ids() {
        let frame1 = serde_json::to_string(&vec![json!([
            "wrb.fr",
            null,
            "[null,[null,\"r_e0e4aa76ab2755e3\"],{\"18\":\"r_e0e4aa76ab2755e3\",\"44\":false}]"
        ])])
        .unwrap();
        let frame2 = serde_json::to_string(&vec![json!([
            "wrb.fr",
            null,
            "[null,[\"c_1004db0ef60d9dda\",\"r_e0e4aa76ab2755e3\"],null,null,[]]"
        ])])
        .unwrap();
        let body = format!(
            ")]}}'\n\n{}\n{}\n{}\n{}\n",
            frame1.encode_utf16().count(),
            frame1,
            frame2.encode_utf16().count(),
            frame2
        );

        let locator = extract_stream_generate_locator(&body).unwrap();
        assert_eq!(locator.response_id, "r_e0e4aa76ab2755e3");
        assert_eq!(locator.conversation_id, "c_1004db0ef60d9dda");
        assert_eq!(locator.app_path, "/app/1004db0ef60d9dda");
    }

    #[test]
    fn stream_generate_parseable_frame_count_recognizes_short_ack_frames() {
        let body = concat!(
            ")]}'\n\n",
            "13\n",
            "[\"wrb.fr\",null,null,null,null,[13]]\n",
            "8\n",
            "[\"di\",71]\n",
            "38\n",
            "[\"af.httprm\",70,\"-8507264369209331675\",1]\n",
            "25\n",
            "[[\"e\",4,null,null,111]]\n"
        );
        assert!(stream_generate_parseable_frame_count(body) > 0);
    }

    #[test]
    fn stream_generate_indicates_image_edit_async_followup_ready_for_response_id_frame() {
        let body = concat!(
            ")]}'\n\n",
            "126\n",
            "[[\"wrb.fr\",null,\"[null,[null,\\\"r_async123\\\"],{\\\"18\\\":\\\"r_async123\\\",\\\"21\\\":[\\\"token\\\"],\\\"44\\\":true}]\"]]\n"
        );
        assert!(stream_generate_indicates_image_edit_async_followup_ready(
            body
        ));
    }

    #[test]
    fn stream_generate_indicates_image_edit_async_followup_ready_for_progress_frame() {
        let body = concat!(
            ")]}'\n\n",
            "36\n",
            "[[\"wrb.fr\",null,\"[{\\\"37\\\":[4]}]\"]]\n"
        );
        assert!(stream_generate_indicates_image_edit_async_followup_ready(
            body
        ));
    }

    #[test]
    fn stream_generate_indicates_image_edit_async_followup_ready_ignores_short_ack_only() {
        let body = concat!(
            ")]}'\n\n",
            "13\n",
            "[[\"wrb.fr\",null,null,null,null,[13]]]\n"
        );
        assert!(!stream_generate_indicates_image_edit_async_followup_ready(
            body
        ));
    }

    #[test]
    fn stream_generate_is_image_edit_short_ack_recognizes_async_ack_frames() {
        let body = concat!(
            ")]}'\n\n",
            "13\n",
            "[\"wrb.fr\",null,null,null,null,[13]]\n",
            "8\n",
            "[\"di\",71]\n",
            "38\n",
            "[\"af.httprm\",70,\"-8507264369209331675\",1]\n"
        );
        assert!(stream_generate_is_image_edit_short_ack(body));
    }

    #[test]
    fn stream_generate_is_image_edit_short_ack_ignores_single_ack_frame() {
        let body = concat!(
            ")]}'\n\n",
            "13\n",
            "[[\"wrb.fr\",null,null,null,null,[13]]]\n"
        );
        assert!(!stream_generate_is_image_edit_short_ack(body));
    }

    #[test]
    fn extract_stream_generate_media_assets_recovers_image_url() {
        let image_url = "https://example.invalid/generated.png";
        let gen_img_data = json!([[
            null,
            null,
            null,
            [
                null,
                "generation.png",
                "AI 生成",
                image_url,
                null,
                "download-token-123"
            ]
        ]]);
        let candidate_data = json!([null, null, null, null, null, null, null, [[gen_img_data]]]);
        let mut frame = vec![Value::Null; 13];
        frame[12] = candidate_data;
        let frame_json = serde_json::to_string(&vec![Value::Array(frame)]).unwrap();
        let body = format!(
            ")]}}'\n{}\n{}\n",
            frame_json.encode_utf16().count(),
            frame_json
        );

        let assets =
            extract_stream_generate_media_assets(&body, GeminiCanvasMediaOperation::Image).unwrap();
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].kind, "image");
        assert_eq!(assets[0].url, image_url);
        assert_eq!(assets[0].mime_type, "image/png");
        assert_eq!(
            assets[0].download_token.as_deref(),
            Some("download-token-123")
        );
    }

    #[test]
    fn extract_stream_generate_media_assets_recovers_image_url_from_root_candidate_shape() {
        let image_url = "https://example.invalid/generated-root.png";
        let gen_img_data = json!([[null, null, null, [null, null, null, image_url]]]);
        let mut candidate_data = vec![Value::Null; 13];
        candidate_data[12] = json!({
            "7": [[gen_img_data]]
        });
        let frame_json = serde_json::to_string(&vec![Value::Array(candidate_data)]).unwrap();
        let body = format!(
            ")]}}'\n{}\n{}\n",
            frame_json.encode_utf16().count(),
            frame_json
        );

        let assets =
            extract_stream_generate_media_assets(&body, GeminiCanvasMediaOperation::Image).unwrap();
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].kind, "image");
        assert_eq!(assets[0].url, image_url);
        assert_eq!(assets[0].mime_type, "image/png");
    }

    #[test]
    fn extract_stream_generate_media_assets_recovers_video_and_music_urls() {
        let video_url = "https://example.invalid/video.mp4";
        let music_url = "https://example.invalid/music.mp3";
        let thumb_url = "https://example.invalid/thumb.png";
        let video_info = json!([[
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            [thumb_url, video_url]
        ]]);
        let music_data = json!([
            [
                null,
                [
                    null,
                    null,
                    null,
                    null,
                    null,
                    null,
                    null,
                    [thumb_url, music_url]
                ]
            ],
            [
                null,
                [
                    null,
                    null,
                    null,
                    null,
                    null,
                    null,
                    null,
                    [thumb_url, "https://example.invalid/music.mp4"]
                ]
            ]
        ]);

        let mut video_frame = vec![Value::Null; 60];
        video_frame[59] = json!([[[video_info]]]);
        let video_frame_json = serde_json::to_string(&vec![Value::Array(video_frame)]).unwrap();
        let video_body = format!(
            ")]}}'\n{}\n{}\n",
            video_frame_json.encode_utf16().count(),
            video_frame_json
        );
        let video_assets =
            extract_stream_generate_media_assets(&video_body, GeminiCanvasMediaOperation::Video)
                .unwrap();
        assert_eq!(video_assets.len(), 1);
        assert_eq!(video_assets[0].kind, "video");
        assert_eq!(video_assets[0].url, video_url);
        assert_eq!(video_assets[0].mime_type, "video/mp4");

        let mut music_frame = vec![Value::Null; 87];
        music_frame[86] = music_data;
        let music_frame_json = serde_json::to_string(&vec![Value::Array(music_frame)]).unwrap();
        let music_body = format!(
            ")]}}'\n{}\n{}\n",
            music_frame_json.encode_utf16().count(),
            music_frame_json
        );
        let music_assets =
            extract_stream_generate_media_assets(&music_body, GeminiCanvasMediaOperation::Music)
                .unwrap();
        assert_eq!(music_assets.len(), 1);
        assert_eq!(music_assets[0].kind, "audio");
        assert_eq!(music_assets[0].url, music_url);
        assert_eq!(music_assets[0].mime_type, "audio/mpeg");
    }

    #[test]
    fn extract_stream_generate_media_assets_recovers_video_and_music_from_root_candidate_shape() {
        let video_url = "https://example.invalid/root-video.mp4";
        let music_url = "https://example.invalid/root-music.mp3";
        let thumb_url = "https://example.invalid/root-thumb.png";
        let video_info = json!([[
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            [thumb_url, video_url]
        ]]);
        let music_data = json!([
            [
                null,
                [
                    null,
                    null,
                    null,
                    null,
                    null,
                    null,
                    null,
                    [thumb_url, music_url]
                ]
            ],
            [
                null,
                [
                    null,
                    null,
                    null,
                    null,
                    null,
                    null,
                    null,
                    [thumb_url, "https://example.invalid/root-music.mp4"]
                ]
            ]
        ]);

        let mut root_video_candidate = vec![Value::Null; 13];
        root_video_candidate[12] = json!({
            "59": [[[video_info]]]
        });
        let root_video_json =
            serde_json::to_string(&vec![Value::Array(root_video_candidate)]).unwrap();
        let root_video_body = format!(
            ")]}}'\n{}\n{}\n",
            root_video_json.encode_utf16().count(),
            root_video_json
        );
        let video_assets = extract_stream_generate_media_assets(
            &root_video_body,
            GeminiCanvasMediaOperation::Video,
        )
        .unwrap();
        assert_eq!(video_assets.len(), 1);
        assert_eq!(video_assets[0].kind, "video");
        assert_eq!(video_assets[0].url, video_url);
        assert_eq!(video_assets[0].mime_type, "video/mp4");

        let mut root_music_candidate = vec![Value::Null; 13];
        root_music_candidate[12] = json!({
            "86": music_data
        });
        let root_music_json =
            serde_json::to_string(&vec![Value::Array(root_music_candidate)]).unwrap();
        let root_music_body = format!(
            ")]}}'\n{}\n{}\n",
            root_music_json.encode_utf16().count(),
            root_music_json
        );
        let music_assets = extract_stream_generate_media_assets(
            &root_music_body,
            GeminiCanvasMediaOperation::Music,
        )
        .unwrap();
        assert_eq!(music_assets.len(), 1);
        assert_eq!(music_assets[0].kind, "audio");
        assert_eq!(music_assets[0].url, music_url);
        assert_eq!(music_assets[0].mime_type, "audio/mpeg");
    }

    #[test]
    fn extract_stream_generate_media_assets_recovers_music_urls_from_new_87_shape() {
        let music_url =
            "https://contribution.usercontent.google.com/download?filename=ascending_north.mp3";
        let music_preview = "https://lh3.googleusercontent.com/gg-dl/preview-music";
        let video_url =
            "https://contribution.usercontent.google.com/download?filename=ascending_north.mp4";
        let video_preview = "https://lh3.googleusercontent.com/gg-dl/preview-video";
        let music_data = json!([
            [
                null,
                [
                    null,
                    4,
                    "ascending_north.mp3",
                    null,
                    null,
                    "$music-download-token",
                    null,
                    [music_preview, music_url, music_preview],
                    2,
                    [1778420728, 123211014],
                    null,
                    "audio/mpeg",
                    null,
                    null,
                    null,
                    null,
                    [[]]
                ]
            ],
            [
                null,
                [
                    null,
                    2,
                    "ascending_north.mp4",
                    null,
                    null,
                    "$video-download-token",
                    null,
                    [video_preview, video_url, video_preview],
                    3,
                    [1778420728, 223211014],
                    null,
                    "video/mp4",
                    null,
                    null,
                    null,
                    null,
                    [[]]
                ]
            ]
        ]);

        let mut frame = vec![Value::Null; 88];
        frame[87] = music_data;
        let frame_json = serde_json::to_string(&vec![Value::Array(frame)]).unwrap();
        let body = format!(
            ")]}}'\n{}\n{}\n",
            frame_json.encode_utf16().count(),
            frame_json
        );

        let assets =
            extract_stream_generate_media_assets(&body, GeminiCanvasMediaOperation::Music).unwrap();
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].kind, "audio");
        assert_eq!(assets[0].url, music_url);
        assert_eq!(assets[0].mime_type, "audio/mpeg");
    }

    #[test]
    fn extract_stream_generate_media_assets_recovers_music_urls_from_wrapped_87_shape() {
        let wrapped_frame = r#"[["wrb.fr",null,"[null,[\"c_f41e19b1a71ccd30\",\"r_b0fc2aaf1a366df8\"],null,null,[[\"rc_572c503ef5e46c34\",[\"\"],null,null,null,null,null,null,[1],null,null,null,[{\"8\":[],\"73\":[null,\"Generating your music...\",null,3],\"87\":[[null,[null,4,\"concrete_maps.mp3\",null,null,\"$AT+3-token\",null,[\"https://lh3.googleusercontent.com/gg-dl/preview-music\",\"https://contribution.usercontent.google.com/download?filename=concrete_maps.mp3\",\"https://lh3.googleusercontent.com/gg-dl/preview-music\"],2,[1778422963,941200288],null,\"audio/mpeg\",null,null,null,null,[[]]]],[null,[null,2,\"concrete_maps.mp4\",null,null,\"$AT+3-video-token\",null,[\"https://lh3.googleusercontent.com/gg-dl/preview-video\",\"https://contribution.usercontent.google.com/download?filename=concrete_maps.mp4\",\"https://lh3.googleusercontent.com/gg-dl/preview-video\"],3,[1778422963,941200288],null,\"video/mp4\",null,null,null,null,[[]]]]]}]]]]"]]"#;

        let body = format!(")]}}'\n{}\n{}\n", wrapped_frame.len(), wrapped_frame);
        let assets =
            extract_stream_generate_media_assets(&body, GeminiCanvasMediaOperation::Music).unwrap();
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].kind, "audio");
        assert_eq!(
            assets[0].url,
            "https://contribution.usercontent.google.com/download?filename=concrete_maps.mp3"
        );
        assert_eq!(assets[0].mime_type, "audio/mpeg");
    }

    #[test]
    fn extract_video_asset_from_candidate_data_falls_back_to_completion_tuple_scan() {
        let asset = extract_video_asset_from_candidate_data(&json!({
            "60": [[[[[
                null,
                2,
                "video.mp4",
                null,
                null,
                "$download-token",
                null,
                [
                    "https://lh3.googleusercontent.com/gg/preview-token",
                    "https://contribution.usercontent.google.com/download?c=xyz789&filename=video.mp4&opi=103135050"
                ],
                2,
                [1775858568, 971367988],
                null,
                "video/mp4",
                null,
                null,
                null,
                null,
                null,
                [[8], 1280, 720]
            ]]]]]
        }))
        .expect("video asset should be recovered from completion tuple scan");

        assert_eq!(asset.kind, "video");
        assert_eq!(asset.mime_type, "video/mp4");
        assert_eq!(
            asset.url,
            "https://contribution.usercontent.google.com/download?c=xyz789&filename=video.mp4&opi=103135050"
        );
    }

    #[test]
    fn extract_music_asset_from_candidate_data_falls_back_to_google_media_url_scan() {
        let asset = extract_music_asset_from_candidate_data(&json!({
            "preview": {
                "urls": [
                    "https://lh3.googleusercontent.com/gg/preview-only",
                    "https://work.fife.usercontent.google.com/rd-gg-dl/abc123/ivory_rain.mp4"
                ]
            }
        }))
        .expect("music asset should be recovered from google media url scan");

        assert_eq!(asset.kind, "video");
        assert_eq!(asset.mime_type, "video/mp4");
        assert_eq!(
            asset.url,
            "https://work.fife.usercontent.google.com/rd-gg-dl/abc123/ivory_rain.mp4"
        );
    }

    #[test]
    fn extract_stream_generate_media_assets_recovers_image_url_when_frame_length_is_invalid() {
        let image_url = "https://example.invalid/generated-invalid-length.png";
        let gen_img_data = json!([[null, null, null, [null, null, null, image_url]]]);
        let candidate_data = json!([null, null, null, null, null, null, null, [[gen_img_data]]]);
        let mut frame = vec![Value::Null; 13];
        frame[12] = candidate_data;
        let frame_json = serde_json::to_string(&vec![Value::Array(frame)]).unwrap();
        let invalid_length = frame_json.encode_utf16().count().saturating_add(7);
        let body = format!(")]}}'\n{}\n{}\n", invalid_length, frame_json);

        let assets =
            extract_stream_generate_media_assets(&body, GeminiCanvasMediaOperation::Image).unwrap();
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].kind, "image");
        assert_eq!(assets[0].url, image_url);
        assert_eq!(assets[0].mime_type, "image/png");
    }

    #[test]
    fn extract_stream_generate_media_assets_recovers_image_url_when_utf16_cut_is_truncated() {
        let image_url = "https://example.invalid/generated-short-length.png";
        let gen_img_data = json!([[null, null, null, [null, null, null, image_url]]]);
        let candidate_data = json!([null, null, null, null, null, null, null, [[gen_img_data]]]);
        let mut frame = vec![Value::Null; 13];
        frame[12] = candidate_data;
        let frame_json = serde_json::to_string(&vec![Value::Array(frame)]).unwrap();
        let truncated_length = frame_json.encode_utf16().count().saturating_sub(9);
        let body = format!(")]}}'\n{}\n{}\n", truncated_length, frame_json);

        let assets =
            extract_stream_generate_media_assets(&body, GeminiCanvasMediaOperation::Image).unwrap();
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].kind, "image");
        assert_eq!(assets[0].url, image_url);
        assert_eq!(assets[0].mime_type, "image/png");
    }

    #[test]
    fn extract_stream_generate_media_assets_returns_image_unavailable_for_blocked_text_body() {
        let body =
            "Are you signed in? I can search images, but I can't seem to create any images for you right now.";
        let error = extract_stream_generate_media_assets(body, GeminiCanvasMediaOperation::Image)
            .unwrap_err();
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_image_generation_unavailable")
        );
    }

    #[test]
    fn extract_stream_generate_media_assets_returns_image_unavailable_for_blocked_frame_body() {
        let blocked = "I can search for images, but can't create any for you at the moment.";
        let frame_json = serde_json::to_string(&vec![Value::Array(vec![Value::String(
            blocked.to_string(),
        )])])
        .unwrap();
        let body = format!(
            ")]}}'\n{}\n{}\n",
            frame_json.encode_utf16().count(),
            frame_json
        );

        let error = extract_stream_generate_media_assets(&body, GeminiCanvasMediaOperation::Image)
            .unwrap_err();
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_image_generation_unavailable")
        );
    }

    #[test]
    fn extract_page_blob_media_assets_recovers_encoded_music_download_url() {
        let body = r#"
            <script>
                window.__DATA__ = [
                    "https:\/\/contribution.usercontent.google.com\/download?c\u003dabc123\u0026filename\u003divory_rain.mp4\u0026opi\u003d103135050"
                ];
            </script>
        "#;

        let assets = extract_page_blob_media_assets(body, GeminiCanvasMediaOperation::Music)
            .expect("music asset should be recovered from page blob");
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].kind, "video");
        assert_eq!(assets[0].mime_type, "video/mp4");
        assert_eq!(
            assets[0].url,
            "https://contribution.usercontent.google.com/download?c=abc123&filename=ivory_rain.mp4&opi=103135050"
        );
    }

    #[test]
    fn extract_page_blob_media_assets_recovers_encoded_video_download_url() {
        let body = r#"
            <video
                src="https:\/\/contribution.usercontent.google.com\/download?c\u003dxyz789\u0026filename\u003dvideo.mp4\u0026opi\u003d103135050">
            </video>
        "#;

        let assets = extract_page_blob_media_assets(body, GeminiCanvasMediaOperation::Video)
            .expect("video asset should be recovered from page blob");
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].kind, "video");
        assert_eq!(assets[0].mime_type, "video/mp4");
        assert_eq!(
            assets[0].url,
            "https://contribution.usercontent.google.com/download?c=xyz789&filename=video.mp4&opi=103135050"
        );
    }

    #[test]
    fn extract_page_blob_media_assets_recovers_work_fife_music_download_url() {
        let body = r#"
            <audio
                src="https:\/\/work.fife.usercontent.google.com\/rd-gg-dl\/abc123\/ivory_rain.mp4">
            </audio>
        "#;

        let assets = extract_page_blob_media_assets(body, GeminiCanvasMediaOperation::Music)
            .expect("music asset should be recovered from work.fife page blob");
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].kind, "video");
        assert_eq!(assets[0].mime_type, "video/mp4");
        assert_eq!(
            assets[0].url,
            "https://work.fife.usercontent.google.com/rd-gg-dl/abc123/ivory_rain.mp4"
        );
    }

    #[test]
    fn extract_page_blob_media_assets_prefers_audio_for_music_when_both_audio_and_video_exist() {
        let body = r#"
            <script>
                window.__DATA__ = [
                    "https:\/\/contribution.usercontent.google.com\/download?c\u003dabc123\u0026filename\u003divory_rain.mp3\u0026opi\u003d103135050",
                    "https:\/\/contribution.usercontent.google.com\/download?c\u003dabc123\u0026filename\u003divory_rain.mp4\u0026opi\u003d103135050"
                ];
            </script>
        "#;

        let assets = extract_page_blob_media_assets(body, GeminiCanvasMediaOperation::Music)
            .expect("music assets should be recovered from page blob");
        assert_eq!(assets.len(), 2);
        assert_eq!(assets[0].kind, "audio");
        assert_eq!(assets[0].mime_type, "audio/mpeg");
        assert_eq!(
            assets[0].url,
            "https://contribution.usercontent.google.com/download?c=abc123&filename=ivory_rain.mp3&opi=103135050"
        );
    }

    #[test]
    fn extract_page_blob_media_assets_recovers_lh3_video_download_url() {
        let body = r#"
            <script>
                window.__VIDEO__ = [
                    "https:\/\/lh3.googleusercontent.com\/gg-dl\/ABCDEF12345\/video.mp4"
                ];
            </script>
        "#;

        let assets = extract_page_blob_media_assets(body, GeminiCanvasMediaOperation::Video)
            .expect("video asset should be recovered from lh3 page blob");
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].kind, "video");
        assert_eq!(assets[0].mime_type, "video/mp4");
        assert_eq!(
            assets[0].url,
            "https://lh3.googleusercontent.com/gg-dl/ABCDEF12345/video.mp4"
        );
    }

    #[test]
    fn extract_page_blob_media_assets_recovers_lh3_rd_ogw_image_url() {
        let body = r#"
            <script>
                window.__IMAGE__ = [
                    "https:\/\/lh3.googleusercontent.com\/rd-ogw\/AF2bZyjVGJ57ttVmI02_SfxKBzom-eSodUpyh2VRJQ9_3HcIhy71DYPyaoB9KrzT8c7l6W4Nw7l7WoKSqnhWlz6RW_VxXyD9WkpkpFmsyva75EH7PKTzGfZIktNW9KbmuqfhEjms3V6AGnWtM4JwmqsPDUzVGGlU22X8hTZSF281orL5hE37kQbf0Jj18yRBse44MCPoJnOuumXyn-ONdceYKQ20Xu52sai3RMnCRPAKYCOlExLjXSJIYTwqmp6ALGdqZNUejShtuXwYsdZspE0G0dkTPyHa2lRBKkVi406x8hjf6_VT40NJ1NaaEpthLCHLTODvp4JfDRXpIFxo_T7JtI6DvfFVSOGoD4NjAizP8u53rgyfNv48b2eXegfMtFi1tRdrEWruepcRE7IqJc383vJ9BOKeLGuB5u0HOdX6_ktB14HY-fVF7Z3vZ8h0wJkgTUXQeEVsYbjTgmt2VIVVkCYy-Wnvbfj9hs7Styyi9mKRtwVmoUtSJsPy9kXrEJAES4Ml1xao4fAY0D1h4JZxqpaaKu65JVGBJpv5xbZ-sqyzrpN09lcnST0aJJPGHYiMMpQYyJySwHZfI2VYPg3P38O1aLhByDMEscyVmxO-VmaPu83_b-FrJem8I63KX6B6cRmmC6-Co56V59kHzJdrPZZR3Xl7QO1qweg576sjaerh73eQjLawbadzn7umSjeZVSp7NFi0ps1daiHafyqH3YCtmEN1gdUDOwATzF088177AG2btc6urscn3sytGl6MPCV4AYec5eDAwYwWzrc-IhSCS1sEP0sQ5PSkqooG0Gft1vsNuyp9kGBT7717M-dLJmyqPHXn8XMujltusg8VhLPX6-FgALkR1EMAVlOZ_HdrytoNHJVmry8MVPb7Etl9OnR5ilqDcyiXYCYMwUatxqSlmjvH9yYLX6c91nY1Jshder2Q86_qb2M5r5uIRAGL85iGSWdGRLQJ14fOykvC0Jwt7J5G4rZTn8_0icvOe8b3itux0nB9up8PnZ1Nm8ne0yrCe8h2SivIDnc55DMcZxjoqAnLt2rDlnStlG2jyPI=s32-c"
                ];
            </script>
        "#;

        let assets = extract_page_blob_media_assets(body, GeminiCanvasMediaOperation::Image)
            .expect("image asset should be recovered from lh3 rd-ogw page blob");
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].kind, "image");
        assert_eq!(assets[0].mime_type, "image/png");
        assert!(assets[0]
            .url
            .starts_with("https://lh3.googleusercontent.com/rd-ogw/"));
    }

    #[test]
    fn extract_video_generation_job_id_reads_uuid_from_pending_body() {
        let body = r#"
            )]}'
            [["wrb.fr",null,"[null,[\"c_98256aabc450ff93\",\"r_931fae4dfff02e00\"],null,null,[[\"rc_pending\",[\"正在生成视频，这可能需要几分钟时间，请稍后回来查看完成状态。\nhttp://googleusercontent.com/video_gen_chip/0\n\"],null,null,null,null,null,null,[1],\"zh\",null,null,[{\"65\":[[\"http://googleusercontent.com/video_gen_chip/0\"],\"e3136f2f-29f4-4f33-9223-271d4697c60a\"]}]]]]"]]
        "#;

        let job_id = extract_video_generation_job_id(body)
            .expect("video generation job id should be recovered from pending response");
        assert_eq!(job_id, "e3136f2f-29f4-4f33-9223-271d4697c60a");
        assert!(response_indicates_video_generation_pending(body));
    }

    #[test]
    fn response_indicates_video_generation_pending_for_async_contract_ready_frames() {
        let body = r#"
            )]}'
            [["wrb.fr",null,"[null,[null,\"r_7013976975ffa957\"],{\"18\":\"r_7013976975ffa957\",\"21\":[\"8X5YJX-Z3wmpPmdEQeUnP4LP6yxOur647qqxisHb4vM\"],\"44\":true}]"]]
            [["wrb.fr",null,null,null,null,[8,null,[[\"type.googleapis.com/assistant.boq.bard.application.BardErrorInfo\",[1053]]]]]
        "#;

        assert!(response_indicates_video_generation_pending(body));
    }

    #[test]
    fn build_video_metadata_followup_request_matches_capture_payload() {
        let bootstrap = gemini_web::GeminiWebBootstrap {
            access_token: Some("at-1".to_string()),
            build_label: Some("bl-1".to_string()),
            session_id: Some("sid-1".to_string()),
            language: "zh-CN".to_string(),
            push_id: None,
            client_pctx: None,
            app_page_path: None,
        };

        let request = build_video_metadata_followup_request(
            &bootstrap,
            "/app/98256aabc450ff93",
            "c_98256aabc450ff93",
            "r_931fae4dfff02e00",
        )
        .unwrap();

        assert!(request
            .query
            .iter()
            .any(|(key, value)| key == "rpcids" && value == "MUAZcd"));
        let f_req = request
            .form
            .iter()
            .find(|(key, _)| key == "f.req")
            .map(|(_, value)| value)
            .unwrap();
        let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
        let rpc = outer[0].as_array().and_then(|items| items.first()).unwrap();
        let rpc_items = rpc.as_array().unwrap();
        assert_eq!(rpc_items[0].as_str(), Some("MUAZcd"));

        let payload = serde_json::from_str::<Value>(rpc_items[1].as_str().unwrap()).unwrap();
        assert_eq!(
            payload,
            json!([
                null,
                [["unread_metadata"]],
                [
                    "c_98256aabc450ff93",
                    null,
                    null,
                    null,
                    null,
                    null,
                    [["c_98256aabc450ff93", "r_931fae4dfff02e00"], 0]
                ]
            ])
        );
    }

    #[test]
    fn response_indicates_image_generation_unavailable_detects_english_blocked_message() {
        let body = "Are you signed in? I can search images, but I can't seem to create any images for you right now.";
        assert!(response_indicates_image_generation_unavailable(
            200,
            Some("application/json; charset=utf-8"),
            body,
        ));
    }

    #[test]
    fn response_indicates_image_generation_unavailable_detects_chinese_region_message() {
        let body = "您登录了吗？我可以搜索图片，但目前似乎无法为您创建任何图片。也有可能您所在的地区尚未开通图片创建功能。";
        assert!(response_indicates_image_generation_unavailable(
            200,
            Some("application/json; charset=utf-8"),
            body,
        ));
    }

    #[test]
    fn response_indicates_image_generation_unavailable_ignores_real_image_artifact_body() {
        let body = "image_generation_content https://lh3.googleusercontent.com/gg-dl/example";
        assert!(!response_indicates_image_generation_unavailable(
            200,
            Some("application/json; charset=utf-8"),
            body,
        ));
    }

    #[test]
    fn normalize_video_generations_defaults_model() {
        let req = normalize_video_generations(json!({
            "prompt": "a corgi running on the moon"
        }))
        .unwrap();

        assert_eq!(req.endpoint_kind, EndpointKind::VideosGenerations);
        assert_eq!(
            req.requested_model.as_deref(),
            Some(GEMINI_CANVAS_VIDEO_PREVIEW_MODEL)
        );
        assert_eq!(
            req.messages[0].text_content(),
            "a corgi running on the moon"
        );
    }

    #[test]
    fn resolve_music_model_accepts_preview_id() {
        assert_eq!(
            resolve_music_model(GEMINI_CANVAS_MUSIC_PREVIEW_MODEL).unwrap(),
            GEMINI_CANVAS_MUSIC_PREVIEW_MODEL
        );
    }

    #[test]
    fn resolve_official_media_models_map_canvas_aliases() {
        assert_eq!(
            resolve_official_image_model(GEMINI_25_FLASH_IMAGE_PREVIEW_MODEL).unwrap(),
            GEMINI_CANVAS_OFFICIAL_IMAGE_MODEL
        );
        assert_eq!(
            resolve_official_image_model(GEMINI_31_FLASH_IMAGE_PREVIEW_MODEL).unwrap(),
            GEMINI_CANVAS_OFFICIAL_IMAGE_MODEL_PREVIEW
        );
        assert_eq!(
            resolve_official_music_model(GEMINI_CANVAS_MUSIC_PREVIEW_MODEL).unwrap(),
            GEMINI_CANVAS_OFFICIAL_MUSIC_MODEL
        );
        assert_eq!(
            resolve_official_video_model(GEMINI_CANVAS_VIDEO_PREVIEW_MODEL).unwrap(),
            GEMINI_CANVAS_OFFICIAL_VIDEO_MODEL
        );
    }

    #[test]
    fn resolve_direct_http_image_model_preserves_canvas_preview_aliases() {
        assert_eq!(
            resolve_direct_http_image_model(GEMINI_25_FLASH_IMAGE_PREVIEW_MODEL).unwrap(),
            GEMINI_25_FLASH_IMAGE_PREVIEW_MODEL
        );
        assert_eq!(
            resolve_direct_http_image_model(GEMINI_25_FLASH_IMAGE_MODEL).unwrap(),
            GEMINI_25_FLASH_IMAGE_MODEL
        );
        assert_eq!(
            resolve_direct_http_image_model(GEMINI_31_FLASH_IMAGE_PREVIEW_MODEL).unwrap(),
            GEMINI_31_FLASH_IMAGE_PREVIEW_MODEL
        );
    }

    #[test]
    fn build_direct_http_image_request_body_uses_text_and_image_modalities() {
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ImagesGenerations,
            requested_model: Some(GEMINI_25_FLASH_IMAGE_PREVIEW_MODEL.to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "paint a skyline".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({
                "model": GEMINI_25_FLASH_IMAGE_PREVIEW_MODEL,
                "prompt": "paint a skyline",
                "response_format": "url",
                "size": "1024x1024",
            }),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        };

        let body = build_direct_http_image_request_body(&req, GEMINI_25_FLASH_IMAGE_PREVIEW_MODEL);
        assert_eq!(
            body["generationConfig"]["responseModalities"],
            json!(["TEXT", "IMAGE"])
        );
        assert_eq!(
            body["generationConfig"]["imageConfig"]["aspectRatio"],
            json!("1:1")
        );
        assert!(body.get("model").is_none());
    }

    #[test]
    fn build_image_request_body_rewrites_data_url_inputs_to_inline_data() {
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ImagesEdits,
            requested_model: Some(GEMINI_25_FLASH_IMAGE_PREVIEW_MODEL.to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![
                    ContentPart::ImageUrl {
                        image_url: "data:image/png;base64,aGVsbG8=".to_string(),
                        detail: None,
                    },
                    ContentPart::Text {
                        text: "edit the sample".to_string(),
                    },
                ],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({
                "model": GEMINI_25_FLASH_IMAGE_PREVIEW_MODEL,
                "prompt": "edit the sample",
                "images": [{
                    "mime_type": "image/png",
                    "base64": "aGVsbG8="
                }],
                "response_format": "url",
            }),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        };

        let body = build_image_request_body(&req, GEMINI_25_FLASH_IMAGE_PREVIEW_MODEL);
        assert_eq!(
            body["contents"][0]["parts"][0]["inlineData"]["mimeType"],
            json!("image/png")
        );
        assert_eq!(
            body["contents"][0]["parts"][0]["inlineData"]["data"],
            json!("aGVsbG8=")
        );
        assert!(body["contents"][0]["parts"][0].get("fileData").is_none());
        assert_eq!(body["contents"][0]["parts"][1]["text"], "edit the sample");
    }

    #[test]
    fn build_imagen_predict_request_uses_prompt_and_aspect_ratio() {
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ImagesGenerations,
            requested_model: Some(GEMINI_25_FLASH_IMAGE_PREVIEW_MODEL.to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "paint a skyline".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({
                "model": GEMINI_25_FLASH_IMAGE_PREVIEW_MODEL,
                "prompt": "paint a skyline",
                "response_format": "url",
                "size": "1536x1024",
                "n": 2,
            }),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        };

        let body = build_imagen_predict_request(&req, "paint a skyline");
        assert_eq!(body["instances"][0]["prompt"], "paint a skyline");
        assert_eq!(body["parameters"]["sampleCount"], 2);
        assert_eq!(body["parameters"]["aspectRatio"], "3:2");
    }

    #[test]
    fn direct_http_image_api_base_url_prefers_clients6_for_google_api_default() {
        let runtime = GeminiCanvasRuntime {
            runtime_state_object_key: "credential-runtime/gemini-canvas/example.json".to_string(),
            share_id: "share-demo".to_string(),
            api_base_url: GEMINI_CANVAS_DEFAULT_API_BASE_URL.to_string(),
        };
        assert_eq!(
            direct_http_image_api_base_url(&runtime),
            GEMINI_CANVAS_DIRECT_HTTP_IMAGE_API_BASE_URL
        );

        let custom_runtime = GeminiCanvasRuntime {
            runtime_state_object_key: runtime.runtime_state_object_key,
            share_id: runtime.share_id,
            api_base_url: "http://localhost:4219/v1beta".to_string(),
        };
        assert_eq!(
            direct_http_image_api_base_url(&custom_runtime),
            "http://localhost:4219/v1beta"
        );
    }

    #[test]
    fn build_openai_images_response_can_emit_direct_urls() {
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ImagesGenerations,
            requested_model: Some(GEMINI_CANVAS_DEFAULT_MODEL.to_string()),
            stream: false,
            messages: vec![],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({
                "prompt": "banana",
                "response_format": "url"
            }),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        };

        let response = build_openai_images_response_from_urls(
            &req,
            "banana",
            &[GeminiCanvasMediaAsset {
                kind: "image".to_string(),
                url: "https://lh3.googleusercontent.com/gg-dl/abc".to_string(),
                mime_type: "image/png".to_string(),
                download_token: None,
                body_base64: None,
                alt: Some("AI 生成".to_string()),
                width: Some(1024),
                height: Some(1024),
                duration_seconds: None,
            }],
        )
        .unwrap();

        assert_eq!(
            response["data"][0]["url"].as_str(),
            Some("https://lh3.googleusercontent.com/gg-dl/abc")
        );
    }

    #[test]
    fn build_music_generation_response_uses_music_object_shape() {
        let response = build_music_generation_response(
            GEMINI_CANVAS_MUSIC_PREVIEW_MODEL,
            "warm lo-fi piano with rain",
            &GeminiCanvasMediaAsset {
                kind: "video".to_string(),
                url: "https://contribution.usercontent.google.com/download?filename=ivory_rain.mp4"
                    .to_string(),
                mime_type: "video/mp4".to_string(),
                download_token: None,
                body_base64: None,
                alt: None,
                width: Some(0),
                height: Some(0),
                duration_seconds: Some(10.0),
            },
            Some("音乐已经准备就绪"),
        );

        assert_eq!(response["object"], "music.generation");
        assert_eq!(response["provider"], "gemini_canvas");
        assert_eq!(response["data"][0]["mime_type"], "video/mp4");
    }

    #[test]
    fn video_body_indicates_music_modality_mismatch_detects_music_markers() {
        let body = "I have created your original live ambient piano track. http://googleusercontent.com/generated_music_content/0 gemini.google.com/music";
        assert!(video_body_indicates_music_modality_mismatch(body));
    }

    #[test]
    fn video_body_indicates_music_modality_mismatch_ignores_regular_video_markers() {
        let body =
            "video.generation completed with generated_video_content/0 and contribution.usercontent.google.com/download?filename=video.mp4";
        assert!(!video_body_indicates_music_modality_mismatch(body));
    }

    #[test]
    fn build_tts_request_body_sets_audio_modality_and_voice() {
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::AudioSpeech,
            requested_model: Some(GEMINI_CANVAS_DEFAULT_TTS_MODEL.to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "say hello".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({
                "input": "say hello",
                "voice": "Kore",
                "response_format": "wav",
            }),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        };

        let body = build_tts_request_body(&req, GEMINI_CANVAS_DEFAULT_TTS_MODEL);
        assert_eq!(
            body["generationConfig"]["responseModalities"],
            json!(["AUDIO"])
        );
        assert_eq!(
            body["generationConfig"]["speechConfig"]["voiceConfig"]["prebuiltVoiceConfig"]
                ["voiceName"],
            "Kore"
        );
        assert!(body.get("model").is_none());
    }

    #[test]
    fn extract_inline_image_from_generate_content_response_reads_inline_data() {
        let image = extract_inline_image_from_generate_content_response(&json!({
            "candidates": [{
                "content": {
                    "parts": [{
                        "inlineData": {
                            "mimeType": "image/png",
                            "data": base64::engine::general_purpose::STANDARD.encode([1u8, 2, 3, 4]),
                        }
                    }]
                }
            }]
        }))
        .unwrap();

        assert_eq!(image.mime_type, "image/png");
        assert_eq!(image.bytes, vec![1, 2, 3, 4]);
    }

    #[test]
    fn extract_images_from_imagen_predict_response_reads_generated_images_shape() {
        let response = json!({
            "generatedImages": [{
                "image": {
                    "mimeType": "image/png",
                    "imageBytes": base64::engine::general_purpose::STANDARD.encode([1u8, 2, 3, 4]),
                }
            }]
        });

        let images = extract_images_from_imagen_predict_response(&response).unwrap();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].mime_type, "image/png");
        assert_eq!(images[0].bytes, vec![1, 2, 3, 4]);
    }

    #[test]
    fn extract_images_from_imagen_predict_response_reads_predictions_shape() {
        let response = json!({
            "predictions": [{
                "mimeType": "image/jpeg",
                "bytesBase64Encoded": base64::engine::general_purpose::STANDARD.encode([9u8, 8, 7]),
            }]
        });

        let images = extract_images_from_imagen_predict_response(&response).unwrap();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].mime_type, "image/jpeg");
        assert_eq!(images[0].bytes, vec![9, 8, 7]);
    }

    #[test]
    fn storage_state_to_pure_http_session_extracts_google_cookie_material() {
        let storage_state = json!({
            "cookies": [
                {
                    "name": "__Secure-1PAPISID",
                    "value": "sapisid-123",
                    "domain": ".google.com",
                    "path": "/"
                },
                {
                    "name": "NID",
                    "value": "nid-123",
                    "domain": ".google.com",
                    "path": "/"
                },
                {
                    "name": "not-for-google",
                    "value": "ignore",
                    "domain": ".example.com",
                    "path": "/"
                }
            ]
        });

        let session = storage_state_to_pure_http_session(
            &storage_state,
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-3-flash-preview:generateContent",
            "https://gemini.google.com",
            "1",
        )
        .unwrap();

        assert_eq!(session.sapisid, "sapisid-123");
        assert_eq!(session.auth_user, "1");
        assert!(session
            .cookie_header
            .contains("__Secure-1PAPISID=sapisid-123"));
        assert!(session.cookie_header.contains("NID=nid-123"));
        assert!(!session.cookie_header.contains("not-for-google"));
    }

    #[test]
    fn storage_state_to_pure_http_session_uses_fixture_host_fallback_for_google_cookies() {
        let storage_state = json!({
            "cookies": [
                {
                    "name": "SAPISID",
                    "value": "fixture-sapisid-123",
                    "domain": ".google.com",
                    "path": "/"
                },
                {
                    "name": "__Secure-1PSID",
                    "value": "fixture-psid-456",
                    "domain": ".google.com",
                    "path": "/"
                }
            ]
        });

        let session = storage_state_to_pure_http_session(
            &storage_state,
            "http://host.docker.internal:42335/app",
            "http://host.docker.internal:42335",
            "0",
        )
        .unwrap();

        assert_eq!(session.sapisid, "fixture-sapisid-123");
        assert!(session
            .cookie_header
            .contains("SAPISID=fixture-sapisid-123"));
        assert!(session
            .cookie_header
            .contains("__Secure-1PSID=fixture-psid-456"));
    }

    #[test]
    fn direct_http_google_api_key_prefers_payload_extra_body() {
        let mut extra_body = HashMap::new();
        extra_body.insert(
            "googleApiKey".to_string(),
            Value::String("AIzaPayloadKey123".to_string()),
        );
        extra_body.insert(
            "apiKeys".to_string(),
            Value::Array(vec![Value::String("AIzaArrayKey456".to_string())]),
        );
        let payload = ProviderAccountPayload {
            adapter: "gemini_canvas_compatible".to_string(),
            base_url: "https://gemini.google.com".to_string(),
            api_key: "AIzaPayloadKeyZero".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: Some(
                "credential-runtime/gemini-canvas/demo/storage-state.json".to_string(),
            ),
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: Some(GEMINI_CANVAS_DEFAULT_MODEL.to_string()),
            headers: HashMap::new(),
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: None,
            chat_completions_path: None,
            completions_path: None,
            embeddings_path: None,
            audio_transcriptions_path: None,
            audio_speech_path: None,
            messages_path: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            extra_body: Some(extra_body),
            session_auth: None,
            keepalive: None,
        };

        let key = direct_http_google_api_key(
            &payload,
            &json!({
                "apiKeys": ["AIzaStorageKey789"]
            }),
        );
        let keys = direct_http_google_api_keys(
            &payload,
            &json!({
                "apiKeys": ["AIzaStorageKey789"]
            }),
        );

        assert_eq!(key.as_deref(), Some("AIzaPayloadKey123"));
        assert_eq!(
            keys,
            vec![
                "AIzaPayloadKey123".to_string(),
                "AIzaArrayKey456".to_string(),
                "AIzaStorageKey789".to_string()
            ]
        );
    }

    #[test]
    fn direct_http_google_api_key_reads_storage_state_metadata() {
        let payload = ProviderAccountPayload {
            adapter: "gemini_canvas_compatible".to_string(),
            base_url: "https://gemini.google.com".to_string(),
            api_key: "AIzaPayloadKeyZero".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: Some(
                "credential-runtime/gemini-canvas/demo/storage-state.json".to_string(),
            ),
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: Some(GEMINI_CANVAS_DEFAULT_MODEL.to_string()),
            headers: HashMap::new(),
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: None,
            chat_completions_path: None,
            completions_path: None,
            embeddings_path: None,
            audio_transcriptions_path: None,
            audio_speech_path: None,
            messages_path: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            extra_body: Some(HashMap::new()),
            session_auth: None,
            keepalive: None,
        };

        let top_level = direct_http_google_api_key(
            &payload,
            &json!({
                "apiKeys": ["AIzaTopLevel123"]
            }),
        );
        let firebase = direct_http_google_api_key(
            &payload,
            &json!({
                "firebaseConfig": {
                    "apiKey": "AIzaFirebase456"
                }
            }),
        );

        assert_eq!(top_level.as_deref(), Some("AIzaTopLevel123"));
        assert_eq!(firebase.as_deref(), Some("AIzaFirebase456"));
    }

    #[test]
    fn direct_http_google_api_keys_dedupes_all_known_sources() {
        let mut extra_body = HashMap::new();
        extra_body.insert(
            "apiKeys".to_string(),
            Value::Array(vec![
                Value::String("AIzaKeyOne".to_string()),
                Value::String("AIzaKeyTwo".to_string()),
                Value::String("AIzaKeyOne".to_string()),
            ]),
        );
        let payload = ProviderAccountPayload {
            adapter: "gemini_canvas_compatible".to_string(),
            base_url: "https://gemini.google.com".to_string(),
            api_key: "AIzaPayloadKeyZero".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: Some(
                "credential-runtime/gemini-canvas/demo/storage-state.json".to_string(),
            ),
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: Some(GEMINI_CANVAS_DEFAULT_MODEL.to_string()),
            headers: HashMap::new(),
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: None,
            chat_completions_path: None,
            completions_path: None,
            embeddings_path: None,
            audio_transcriptions_path: None,
            audio_speech_path: None,
            messages_path: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            extra_body: Some(extra_body),
            session_auth: None,
            keepalive: None,
        };

        let keys = direct_http_google_api_keys(
            &payload,
            &json!({
                "googleApiKey": "AIzaKeyTwo",
                "apiKeys": ["AIzaKeyThree"],
                "firebaseConfig": {
                    "apiKey": "AIzaKeyFour",
                    "apiKeys": ["AIzaKeyThree", "AIzaKeyFive"]
                }
            }),
        );

        assert_eq!(
            keys,
            vec![
                "AIzaPayloadKeyZero".to_string(),
                "AIzaKeyOne".to_string(),
                "AIzaKeyTwo".to_string(),
                "AIzaKeyThree".to_string(),
                "AIzaKeyFour".to_string(),
                "AIzaKeyFive".to_string()
            ]
        );
    }

    #[test]
    fn extract_google_api_keys_from_page_blob_collects_unique_matches() {
        let blob = r#"
            <html><body>
            <script>
              window.firebaseConfig = {"apiKey":"AIzaBlobKey123456789012345"};
              const more = ["AIzaBlobKeySecond123456789012345", "AIzaBlobKey123456789012345"];
            </script>
            </body></html>
        "#;

        let keys = extract_google_api_keys_from_page_blob(blob);

        assert_eq!(
            keys,
            vec![
                "AIzaBlobKey123456789012345".to_string(),
                "AIzaBlobKeySecond123456789012345".to_string()
            ]
        );
    }

    #[test]
    fn extract_signaler_account_id_from_page_blob_reads_s06grb() {
        let blob = r#"<script>window.WIZ_global_data={"S06Grb":"111226070075366785521"};</script>"#;
        assert_eq!(
            extract_signaler_account_id_from_page_blob(blob).as_deref(),
            Some("111226070075366785521")
        );
    }

    #[test]
    fn build_image_edit_signaler_bodies_match_captured_shape() {
        let choose_server = build_image_edit_signaler_choose_server_body("111226070075366785521");
        assert!(choose_server.contains("\"async_bard\""));
        assert!(choose_server.contains("\"111226070075366785521\""));

        let open_channel = build_image_edit_signaler_open_channel_body("111226070075366785521");
        assert!(open_channel.contains("count=4"));
        assert!(open_channel.contains("req0___data__"));
        assert!(open_channel.contains("async_bard"));
        assert!(open_channel.contains("beyond_agency_bard"));
        assert!(open_channel.contains("mini_app_generation_bard"));
        assert!(open_channel.contains("bard-client-sync"));
        assert!(open_channel.contains("111226070075366785521"));

        let refresh_creds = build_image_edit_signaler_refresh_creds_body("FoFSUpYY");
        assert_eq!(refresh_creds, "[\"FoFSUpYY\"]");
    }

    #[test]
    fn parse_signaler_responses_extract_handshake_and_aid() {
        let choose_body =
            r#"["nELF6cLDQmZXFcvDxW0F45qqGXrhMmLfx6qHkU_Mgkg",3,null,"1777607372305639"]"#;
        assert_eq!(
            parse_signaler_choose_server_response(choose_body).unwrap(),
            "nELF6cLDQmZXFcvDxW0F45qqGXrhMmLfx6qHkU_Mgkg"
        );

        let open_body = "51\n[[0,[\"c\",\"C5GZ7BOxKKfdABZvW9wVmg\",\"\",8,14,30000]]]\n";
        assert_eq!(
            parse_signaler_open_channel_sid(open_body).unwrap(),
            "C5GZ7BOxKKfdABZvW9wVmg"
        );

        let long_poll = concat!(
            "188\n",
            "[[1,[[null,null,[\"d5ty4AOG\"]]]],[2,[[[[\"1\",[[\"1777607372727478\"]]]]]]]]",
            "167\n",
            "[[18,[[[[\"4\",[null,null,[\"1777607613519203\"]]],",
            "[\"3\",[null,null,[\"1777607613519203\"]]]]]]]]"
        );
        assert_eq!(
            extract_signaler_long_poll_refresh_token(long_poll).as_deref(),
            Some("d5ty4AOG")
        );
        assert_eq!(extract_signaler_long_poll_max_aid(long_poll), Some(18));

        let shifted_refresh_long_poll = concat!(
            "187\n",
            "[[1,[[[[\"4\",null,[[16,\"generic\"]]]]]]],[2,[[null,null,[\"UTRNw8fZ\"]]]],[3,[[[[\"1\",[[\"1777678505284506\"]]]]]]]"
        );
        assert_eq!(
            extract_signaler_long_poll_refresh_token(shifted_refresh_long_poll).as_deref(),
            Some("UTRNw8fZ")
        );
    }

    #[test]
    fn extract_signaler_app_paths_recovers_conversation_targets() {
        let body = r#"c_90ca192406be80bb /app/263d92f7707033f8 c_90ca192406be80bb"#;
        assert_eq!(
            extract_signaler_app_paths(body),
            vec![
                "/app/90ca192406be80bb".to_string(),
                "/app/263d92f7707033f8".to_string()
            ]
        );
    }

    #[test]
    fn extract_signaler_app_paths_recovers_decimal_conversation_ids() {
        let body = r#"129 [[21,[[[["3",[null,null,["1777712281763442"]]],["1",[null,null,["1777712281763442"]]]]]]]]"#;
        assert_eq!(
            extract_signaler_app_paths(body),
            vec!["/app/1777712281763442".to_string()]
        );
    }

    #[test]
    fn sapisid_authorization_matches_google_hash_contract() {
        let header =
            build_sapisid_authorization("sapisid-123", "https://gemini.google.com", 1_700_000_000)
                .unwrap();

        assert_eq!(
            header,
            "SAPISIDHASH 1700000000_46035c0888ca202e6ab144cddb345bf95fff120c SAPISID1PHASH 1700000000_46035c0888ca202e6ab144cddb345bf95fff120c SAPISID3PHASH 1700000000_46035c0888ca202e6ab144cddb345bf95fff120c"
        );
    }

    #[test]
    fn extract_audio_from_generate_content_response_reads_inline_data() {
        let audio = extract_audio_from_generate_content_response(&json!({
            "candidates": [{
                "content": {
                    "parts": [{
                        "inlineData": {
                            "mimeType": "audio/L16;codec=pcm;rate=24000",
                            "data": base64::engine::general_purpose::STANDARD.encode([0u8, 1u8, 2u8, 3u8]),
                        }
                    }]
                }
            }]
        }))
        .unwrap();

        assert_eq!(audio.mime_type, "audio/L16;codec=pcm;rate=24000");
        assert_eq!(audio.bytes, vec![0, 1, 2, 3]);
    }

    #[test]
    fn build_audio_binary_response_wraps_pcm_in_wav_by_default() {
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::AudioSpeech,
            requested_model: Some(GEMINI_CANVAS_DEFAULT_TTS_MODEL.to_string()),
            stream: false,
            messages: vec![],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({
                "input": "say hello",
            }),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        };

        let (body, content_type) = build_audio_binary_response(
            &req,
            &GeminiCanvasAudio {
                mime_type: "audio/L16;codec=pcm;rate=24000".to_string(),
                bytes: vec![0x12, 0x34, 0x56, 0x78],
            },
        )
        .unwrap();

        assert_eq!(content_type, "audio/wav");
        assert!(body.starts_with(b"RIFF"));
        assert!(body.windows(4).any(|chunk| chunk == b"WAVE"));
    }

    #[test]
    fn build_audio_binary_response_accepts_opus_payloads() {
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::AudioSpeech,
            requested_model: Some(GEMINI_CANVAS_DEFAULT_TTS_MODEL.to_string()),
            stream: false,
            messages: vec![],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({
                "input": "say hello",
                "response_format": "opus",
            }),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        };

        let (body, content_type) = build_audio_binary_response(
            &req,
            &GeminiCanvasAudio {
                mime_type: "audio/ogg".to_string(),
                bytes: vec![0x4f, 0x67, 0x67, 0x53],
            },
        )
        .unwrap();

        assert_eq!(body, vec![0x4f, 0x67, 0x67, 0x53]);
        assert_eq!(content_type, "audio/ogg");
    }

    #[test]
    fn build_audio_binary_response_rejects_wav_when_only_ogg_is_available() {
        let req = CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::AudioSpeech,
            requested_model: Some(GEMINI_CANVAS_DEFAULT_TTS_MODEL.to_string()),
            stream: false,
            messages: vec![],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({
                "input": "say hello",
                "response_format": "wav",
            }),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        };

        let error = build_audio_binary_response(
            &req,
            &GeminiCanvasAudio {
                mime_type: "audio/ogg".to_string(),
                bytes: vec![0x4f, 0x67, 0x67, 0x53],
            },
        )
        .unwrap_err();

        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_tts_wav_unavailable")
        );
    }

    #[test]
    fn unsupported_media_adapter_endpoint_error_matches_contract() {
        let error = unsupported_media_adapter_endpoint_error("gemini_canvas_compatible");
        assert_eq!(error.http_status, Some(400));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("unsupported_gemini_canvas_endpoint")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas adapters currently support /v1/images/generations, /v1/images/edits, /v1/music/generations, and /v1/videos/generations."
        );
    }

    #[test]
    fn gemini_canvas_unsupported_request_plan_error_matches_contract() {
        let error = unsupported_request_plan_error();
        assert_eq!(error.http_status, Some(400));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("unsupported_gemini_canvas_endpoint")
        );
    }

    #[test]
    fn plan_gemini_canvas_chat_endpoint_rejected_locally() {
        let payload = pure_http_mode_payload(None);
        let req = make_request(EndpointKind::ChatCompletions);
        let err = crate::upstream::client::UpstreamClient::build_request_plan(
            &payload,
            &req,
            GEMINI_CANVAS_DEFAULT_MODEL,
            false,
        )
        .expect_err("gemini canvas chat requests should be rejected");
        assert_eq!(err.http_status, Some(400));
        assert_eq!(
            err.code.as_deref(),
            Some("unsupported_gemini_canvas_endpoint")
        );
    }

    #[test]
    fn unsupported_modular_endpoint_error_matches_contract() {
        let error = unsupported_modular_endpoint_error("gemini_canvas_compatible");
        assert_eq!(error.http_status, Some(400));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("unsupported_gemini_canvas_modular_endpoint")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas modular browser relay currently supports /v1/images/generations, /v1/music/generations, /v1/videos/generations, and /v1/audio/speech."
        );
    }

    #[test]
    fn unsupported_image_count_error_matches_contract() {
        let error = unsupported_image_count_error("gemini_canvas_compatible");
        assert_eq!(error.http_status, Some(400));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("unsupported_gemini_canvas_image_count")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas image generation currently supports only n=1 requests."
        );
    }

    #[test]
    fn unsupported_modular_image_count_error_matches_contract() {
        let error = unsupported_modular_image_count_error("gemini_canvas_compatible");
        assert_eq!(error.http_status, Some(400));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("unsupported_gemini_canvas_modular_image_count")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas browser relay image generation currently supports only n=1 requests."
        );
    }

    #[test]
    fn unsupported_image_edit_count_error_matches_contract() {
        let error = unsupported_image_edit_count_error("gemini_canvas_compatible");
        assert_eq!(error.http_status, Some(400));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("unsupported_gemini_canvas_image_edit_count")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas image edits currently support only n=1 requests."
        );
    }

    #[test]
    fn unsupported_modular_image_edits_error_matches_contract() {
        let error = unsupported_modular_image_edits_error("gemini_canvas_compatible");
        assert_eq!(error.http_status, Some(400));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("unsupported_gemini_canvas_modular_image_edits")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas modular browser relay does not implement image edits yet. Keep using the legacy mixed lane until the true browser-owned edit flow is split out."
        );
    }
}
