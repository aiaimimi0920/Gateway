//! Captured model, browser, and request defaults for the Canvas wire protocol.

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
