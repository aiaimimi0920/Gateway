use super::*;

#[path = "canvas_preflight_part_2/media_capture.rs"]
mod media_capture;
#[path = "canvas_preflight_part_2/text_preflight.rs"]
mod text_preflight;

impl UpstreamClient {
    pub(super) async fn execute_gemini_canvas_image_capture_parity_preflight_sequence(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        source_path: &str,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        batchexecute_header_id: Option<&str>,
        timeout: Duration,
    ) -> Result<(), GatewayError> {
        let neutral_header = gemini_canvas::build_text_batchexecute_model_header_variant(
            None,
            batchexecute_header_id,
            false,
            false,
        );
        let empty_model_header = gemini_canvas::build_text_batchexecute_model_header_variant(
            None,
            batchexecute_header_id,
            false,
            true,
        );
        let flagged_neutral_header = gemini_canvas::build_text_batchexecute_model_header_variant(
            None,
            batchexecute_header_id,
            true,
            false,
        );
        let flagged_selected_header = gemini_canvas::build_text_batchexecute_model_header_variant(
            Some(gemini_canvas::GEMINI_CANVAS_TEXT_SELECTED_MODEL_HEADER_ID),
            batchexecute_header_id,
            true,
            false,
        );
        let language = bootstrap.language.clone();

        for (rpcid, rpc_payload, model_header) in [
            ("otAQ7b", json!([]), neutral_header.as_str()),
            ("sJBwce", json!([[1, 2]]), neutral_header.as_str()),
            ("DYBcR", json!([language.clone()]), neutral_header.as_str()),
            ("aPya6c", json!([]), empty_model_header.as_str()),
            ("cYRIkd", json!([language.clone()]), neutral_header.as_str()),
        ] {
            let request = gemini_canvas::build_text_batchexecute_request(
                rpcid,
                rpc_payload,
                bootstrap,
                source_path,
            )?;
            self.send_gemini_canvas_text_batchexecute_request(
                payload,
                model,
                &request,
                session,
                model_header,
                timeout,
            )
            .await?;
        }

        let early_side_nav = gemini_canvas::build_text_state_variant_preflight_request(
            bootstrap,
            source_path,
            41usize,
            40usize,
            Value::from(0),
            "side_nav_open_by_default",
            gemini_canvas::GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID,
        )?;
        self.send_gemini_canvas_text_batchexecute_request(
            payload,
            model,
            &early_side_nav,
            session,
            &neutral_header,
            timeout,
        )
        .await?;

        let image_state_keys =
            gemini_canvas::build_image_state_keys_preflight_request(bootstrap, source_path)?;
        self.send_gemini_canvas_text_batchexecute_request(
            payload,
            model,
            &image_state_keys,
            session,
            &neutral_header,
            timeout,
        )
        .await?;

        for (rpcid, rpc_payload) in [
            ("ESY5D", json!([[["bard_activity_enabled"]]])),
            ("MaZiqc", json!([13, null, [1, null, 1]])),
            ("MaZiqc", json!([13, null, [0, null, 1]])),
            ("GPRiHf", json!([])),
            ("maGuAc", json!([0])),
            ("maGuAc", json!([1])),
            (
                "qpEbW",
                json!([[
                    [
                        1,
                        gemini_canvas::GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX
                    ],
                    [
                        2,
                        gemini_canvas::GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX
                    ],
                    [
                        6,
                        gemini_canvas::GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX
                    ]
                ]]),
            ),
            ("mhs1xe", json!([[1, 3]])),
            (
                "o30O0e",
                json!([
                    ["me"],
                    [
                        [["person.photo", "person.name", "person.email"]],
                        null,
                        [1, 7]
                    ]
                ]),
            ),
            ("Te6DCf", json!([[language.clone()], [1]])),
            ("K4WWud", json!([[0], [language.clone()]])),
            ("CNgdBe", json!([1, [language.clone()], 0])),
            (
                "ozz5Z",
                json!([[
                    [[null, "1", 447]],
                    [[null, "1", 448]],
                    [[null, "1", 702]],
                    [[null, "1", 961]],
                    [[null, "1", 960]],
                    [[null, "1", 1062]],
                    [[null, "1", 1240]],
                    [[null, "1", 1237]],
                    [[null, "1", 1238]],
                    [[null, "1", 1239]],
                    [[null, "1", 1241]],
                ]]),
            ),
            ("CNgdBe", json!([2, [language.clone()], 0])),
        ] {
            let request = gemini_canvas::build_text_batchexecute_request(
                rpcid,
                rpc_payload,
                bootstrap,
                source_path,
            )?;
            self.send_gemini_canvas_text_batchexecute_request(
                payload,
                model,
                &request,
                session,
                &neutral_header,
                timeout,
            )
            .await?;
        }

        let selected_mode = gemini_canvas::build_mode_selection_preflight_request(
            bootstrap,
            source_path,
            gemini_canvas::GEMINI_CANVAS_TEXT_SELECTED_MODEL_HEADER_ID,
        )?;
        self.send_gemini_canvas_text_batchexecute_request(
            payload,
            model,
            &selected_mode,
            session,
            &flagged_neutral_header,
            timeout,
        )
        .await?;

        let selected_bootstrap =
            gemini_canvas::build_text_bootstrap_preflight_request(bootstrap, source_path)?;
        self.send_gemini_canvas_text_batchexecute_request(
            payload,
            model,
            &selected_bootstrap,
            session,
            &flagged_selected_header,
            timeout,
        )
        .await?;

        for request in [
            gemini_canvas::build_text_state_variant_preflight_request(
                bootstrap,
                source_path,
                41usize,
                40usize,
                Value::from(0),
                "side_nav_open_by_default",
                gemini_canvas::GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID,
            )?,
            gemini_canvas::build_text_state_variant_preflight_request(
                bootstrap,
                source_path,
                41usize,
                40usize,
                Value::from(0),
                "side_nav_open_by_default",
                gemini_canvas::GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID,
            )?,
            gemini_canvas::build_text_state_variant_preflight_request(
                bootstrap,
                source_path,
                87usize,
                86usize,
                Value::from(14),
                "popup_zs_visits_cooldown",
                gemini_canvas::GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID,
            )?,
        ] {
            self.send_gemini_canvas_text_batchexecute_request(
                payload,
                model,
                &request,
                session,
                &flagged_neutral_header,
                timeout,
            )
            .await?;
        }

        let ku4jyf_first = gemini_canvas::build_text_batchexecute_request(
            "ku4Jyf",
            json!([[
                language.clone(),
                null,
                null,
                null,
                gemini_canvas::GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX,
                null,
                null,
                [32],
                null,
                []
            ]]),
            bootstrap,
            source_path,
        )?;
        self.send_gemini_canvas_text_batchexecute_request(
            payload,
            model,
            &ku4jyf_first,
            session,
            &flagged_neutral_header,
            timeout,
        )
        .await?;

        for request in [
            gemini_canvas::build_text_state_variant_preflight_request(
                bootstrap,
                source_path,
                94usize,
                93usize,
                Value::String("NULL".to_string()),
                "current_popup_id",
                gemini_canvas::GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID,
            )?,
            gemini_canvas::build_text_state_variant_preflight_request(
                bootstrap,
                source_path,
                87usize,
                86usize,
                Value::from(15),
                "popup_zs_visits_cooldown",
                gemini_canvas::GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID,
            )?,
            gemini_canvas::build_text_state_variant_preflight_request(
                bootstrap,
                source_path,
                192usize,
                191usize,
                json!([["image_generation_soft:1", "music_generation_soft:1"]]),
                "tool_menu_soft_badge_impression_counts",
                gemini_canvas::GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID,
            )?,
        ] {
            self.send_gemini_canvas_text_batchexecute_request(
                payload,
                model,
                &request,
                session,
                &flagged_neutral_header,
                timeout,
            )
            .await?;
        }

        for (rpcid, rpc_payload) in [
            ("CNgdBe", json!([2, [language], 0, null, [2]])),
            ("XhaU0b", json!([4, [2], 3])),
        ] {
            let request = gemini_canvas::build_text_batchexecute_request(
                rpcid,
                rpc_payload,
                bootstrap,
                source_path,
            )?;
            self.send_gemini_canvas_text_batchexecute_request(
                payload,
                model,
                &request,
                session,
                &flagged_neutral_header,
                timeout,
            )
            .await?;
        }

        Ok(())
    }

    pub(super) async fn execute_gemini_canvas_media_legacy_preflight_sequence(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        source_path: &str,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        mode_index: i64,
        batchexecute_header_id: Option<&str>,
        timeout: Duration,
    ) -> Result<(), GatewayError> {
        self.execute_gemini_canvas_page_init_preflight_sequence(
            payload,
            model,
            bootstrap,
            source_path,
            session,
            timeout,
            true,
        )
        .await?;
        self.execute_gemini_canvas_mode_selection_preflight(
            payload,
            model,
            bootstrap,
            source_path,
            session,
            gemini_canvas::GEMINI_CANVAS_TEXT_SELECTED_MODEL_HEADER_ID,
            batchexecute_header_id,
            timeout,
        )
        .await?;
        self.execute_gemini_canvas_media_operation_selection_preflight(
            payload,
            model,
            bootstrap,
            source_path,
            session,
            mode_index,
            batchexecute_header_id,
            timeout,
        )
        .await?;
        self.execute_gemini_canvas_media_state_preflight_sequence(
            payload,
            model,
            bootstrap,
            source_path,
            session,
            mode_index,
            batchexecute_header_id,
            timeout,
        )
        .await?;
        self.execute_gemini_canvas_text_bootstrap_preflight(
            payload,
            model,
            bootstrap,
            source_path,
            session,
            "",
            "",
            timeout,
        )
        .await?;
        self.execute_gemini_canvas_text_state_preflight(
            payload,
            model,
            bootstrap,
            source_path,
            session,
            "",
            "",
            timeout,
        )
        .await
    }
}
