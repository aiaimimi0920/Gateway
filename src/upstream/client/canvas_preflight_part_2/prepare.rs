use super::*;

impl UpstreamClient {
    pub(super) async fn prepare_media_capture_parity_preflight(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        source_path: &str,
        session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
        mode_index: i64,
        batchexecute_header_id: Option<&str>,
        is_image_edit_request: bool,
        timeout: Duration,
    ) -> Result<MediaCaptureParityPreflightOutcome, GatewayError> {
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
        let selected_header = gemini_canvas::build_text_batchexecute_model_header_variant(
            Some(gemini_canvas::GEMINI_CANVAS_TEXT_SELECTED_MODEL_HEADER_ID),
            batchexecute_header_id,
            false,
            false,
        );
        let language = bootstrap.language.clone();
        let selection_mode_index =
            gemini_canvas::media_operation_selection_preflight_mode_index(mode_index);
        let primary_mode_index = gemini_canvas::media_primary_ku4jyf_mode_index(mode_index);
        let tool_menu_soft_badge_impression_counts =
            if mode_index == gemini_canvas::GEMINI_CANVAS_STREAM_GENERATE_MUSIC_MODE_INDEX {
                json!([["image_generation_soft:4", "music_generation_soft:4"]])
            } else {
                json!([["image_generation_soft:1", "music_generation_soft:1"]])
            };
        let popup_zs_visits_cooldown =
            if mode_index == gemini_canvas::GEMINI_CANVAS_STREAM_GENERATE_MUSIC_MODE_INDEX {
                Value::from(4)
            } else {
                Value::from(15)
            };
        let xhau0b_payload =
            if mode_index == gemini_canvas::GEMINI_CANVAS_STREAM_GENERATE_MUSIC_MODE_INDEX {
                json!([4, [3], 3])
            } else {
                json!([4, [2], 3])
            };
        let is_image_mode =
            mode_index == gemini_canvas::GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX;
        let initial_header2 = "[]";
        let selected_header2 = gemini_canvas::GEMINI_CANVAS_TEXT_PREFLIGHT_MODEL_HEADER_2;
        let mut maziqc_probe_body = None;
        let mut maziqc_full_body = None;
        let mut o30o0e_body = None;
        let mut k4wwud_body = None;

        if is_image_mode && is_image_edit_request {
            let ozz5z_payload = json!([[
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
            ]]);
            let ku4jyf_payload = json!([[
                language.clone(),
                null,
                null,
                null,
                primary_mode_index,
                null,
                null,
                [32],
                null,
                []
            ]]);
            for (rpcid, rpc_payload, model_header) in [
                ("otAQ7b", json!([]), neutral_header.as_str()),
                ("GPRiHf", json!([]), neutral_header.as_str()),
                ("DYBcR", json!([language.clone()]), neutral_header.as_str()),
                (
                    "Te6DCf",
                    json!([[language.clone()], [1]]),
                    neutral_header.as_str(),
                ),
            ] {
                let request = gemini_canvas::build_text_batchexecute_request(
                    rpcid,
                    rpc_payload,
                    bootstrap,
                    source_path,
                )?;
                self.send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
                    payload,
                    model,
                    &request,
                    session,
                    model_header,
                    None,
                    timeout,
                )
                .await?;
            }

            let media_state_keys =
                gemini_canvas::build_image_state_keys_preflight_request(bootstrap, source_path)?;
            self.send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
                payload,
                model,
                &media_state_keys,
                session,
                &neutral_header,
                None,
                timeout,
            )
            .await?;

            for (rpcid, rpc_payload, model_header) in [
                ("aPya6c", json!([]), empty_model_header.as_str()),
                (
                    "ESY5D",
                    json!([[["bard_activity_enabled"]]]),
                    neutral_header.as_str(),
                ),
                (
                    "MaZiqc",
                    json!([13, null, [1, null, 1]]),
                    neutral_header.as_str(),
                ),
                ("sJBwce", json!([[1, 2]]), neutral_header.as_str()),
                ("maGuAc", json!([1]), neutral_header.as_str()),
                ("mhs1xe", json!([[1, 3]]), neutral_header.as_str()),
                (
                    "MaZiqc",
                    json!([13, null, [0, null, 1]]),
                    neutral_header.as_str(),
                ),
                ("ozz5Z", ozz5z_payload.clone(), neutral_header.as_str()),
                ("cYRIkd", json!([language.clone()]), neutral_header.as_str()),
                (
                    "qpEbW",
                    json!([[
                        [1, selection_mode_index],
                        [2, selection_mode_index],
                        [6, selection_mode_index]
                    ]]),
                    neutral_header.as_str(),
                ),
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
                    neutral_header.as_str(),
                ),
                (
                    "K4WWud",
                    json!([[1], [language.clone()]]),
                    neutral_header.as_str(),
                ),
            ] {
                let request = gemini_canvas::build_text_batchexecute_request(
                    rpcid,
                    rpc_payload,
                    bootstrap,
                    source_path,
                )?;
                let body = self
                    .send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
                        payload,
                        model,
                        &request,
                        session,
                        model_header,
                        None,
                        timeout,
                    )
                    .await?;
                match rpcid {
                    "MaZiqc" if maziqc_probe_body.is_none() => {
                        maziqc_probe_body = Some(body);
                    }
                    "MaZiqc" if maziqc_full_body.is_none() => {
                        maziqc_full_body = Some(body);
                    }
                    "o30O0e" => {
                        o30o0e_body = Some(body);
                    }
                    "K4WWud" => {
                        k4wwud_body = Some(body);
                    }
                    _ => {}
                }
            }

            let side_nav_request = gemini_canvas::build_text_state_variant_preflight_request(
                bootstrap,
                source_path,
                41usize,
                40usize,
                Value::from(0),
                "side_nav_open_by_default",
                gemini_canvas::GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID,
            )?;
            self.send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
                payload,
                model,
                &side_nav_request,
                session,
                &neutral_header,
                None,
                timeout,
            )
            .await?;

            let request = gemini_canvas::build_text_batchexecute_request(
                "CNgdBe",
                json!([1, [bootstrap.language.clone()], 0]),
                bootstrap,
                source_path,
            )?;
            self.send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
                payload,
                model,
                &request,
                session,
                &neutral_header,
                None,
                timeout,
            )
            .await?;

            let selected_bootstrap_request =
                gemini_canvas::build_text_bootstrap_preflight_request(bootstrap, source_path)?;
            let selected_bootstrap_body = self
                .send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
                    payload,
                    model,
                    &selected_bootstrap_request,
                    session,
                    &selected_header,
                    None,
                    timeout,
                )
                .await?;

            for (rpcid, rpc_payload) in [
                ("ku4Jyf", ku4jyf_payload.clone()),
                ("CNgdBe", json!([2, [bootstrap.language.clone()], 0])),
            ] {
                let request = gemini_canvas::build_text_batchexecute_request(
                    rpcid,
                    rpc_payload,
                    bootstrap,
                    source_path,
                )?;
                self.send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
                    payload,
                    model,
                    &request,
                    session,
                    &neutral_header,
                    None,
                    timeout,
                )
                .await?;
            }

            let selected_mode = gemini_canvas::build_mode_selection_preflight_request(
                bootstrap,
                source_path,
                gemini_canvas::GEMINI_CANVAS_TEXT_SELECTED_MODEL_HEADER_ID,
            )?;
            self.send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
                payload,
                model,
                &selected_mode,
                session,
                &neutral_header,
                None,
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
                    41usize,
                    40usize,
                    Value::from(0),
                    "side_nav_open_by_default",
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
                self.send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
                    payload,
                    model,
                    &request,
                    session,
                    &neutral_header,
                    None,
                    timeout,
                )
                .await?;
            }

            let xhau0b_request = gemini_canvas::build_text_batchexecute_request(
                "XhaU0b",
                json!([4, [2], 3]),
                bootstrap,
                source_path,
            )?;
            self.send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
                payload,
                model,
                &xhau0b_request,
                session,
                &neutral_header,
                None,
                timeout,
            )
            .await?;

            let final_activity_request = gemini_canvas::build_text_batchexecute_request(
                "ESY5D",
                json!([[["bard_activity_enabled"]]]),
                bootstrap,
                source_path,
            )?;
            self.send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
                payload,
                model,
                &final_activity_request,
                session,
                &neutral_header,
                None,
                timeout,
            )
            .await?;

            return Ok(MediaCaptureParityPreflightOutcome::Completed(
                GeminiCanvasMediaCaptureParityPreflightResult {
                    selected_bootstrap_body,
                    maziqc_probe_body,
                    maziqc_full_body,
                    o30o0e_body,
                    k4wwud_body,
                },
            ));
        }

        let initial_requests: Vec<(&str, Value, &str)> = if is_image_mode {
            vec![
                ("aPya6c", json!([]), empty_model_header.as_str()),
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
                    neutral_header.as_str(),
                ),
                ("GPRiHf", json!([]), neutral_header.as_str()),
                ("otAQ7b", json!([]), neutral_header.as_str()),
                (
                    "qpEbW",
                    json!([[
                        [1, selection_mode_index],
                        [2, selection_mode_index],
                        [6, selection_mode_index]
                    ]]),
                    neutral_header.as_str(),
                ),
                ("cYRIkd", json!([language.clone()]), neutral_header.as_str()),
            ]
        } else {
            vec![
                ("otAQ7b", json!([]), neutral_header.as_str()),
                ("sJBwce", json!([[1, 2]]), neutral_header.as_str()),
                ("DYBcR", json!([language.clone()]), neutral_header.as_str()),
                ("aPya6c", json!([]), empty_model_header.as_str()),
                ("cYRIkd", json!([language.clone()]), neutral_header.as_str()),
            ]
        };

        for (rpcid, rpc_payload, model_header) in initial_requests {
            let request = gemini_canvas::build_text_batchexecute_request(
                rpcid,
                rpc_payload,
                bootstrap,
                source_path,
            )?;
            let body = self
                .send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
                    payload,
                    model,
                    &request,
                    session,
                    model_header,
                    Some(initial_header2),
                    timeout,
                )
                .await?;
            if rpcid == "o30O0e" {
                o30o0e_body = Some(body);
            }
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
        self.send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
            payload,
            model,
            &early_side_nav,
            session,
            &neutral_header,
            Some(initial_header2),
            timeout,
        )
        .await?;
        Ok(MediaCaptureParityPreflightOutcome::Continue(
            MediaCaptureParityPreflightState {
                neutral_header,
                flagged_neutral_header,
                flagged_selected_header,
                language,
                selection_mode_index,
                primary_mode_index,
                tool_menu_soft_badge_impression_counts,
                popup_zs_visits_cooldown,
                xhau0b_payload,
                is_image_mode,
                initial_header2,
                selected_header2,
                maziqc_probe_body,
                maziqc_full_body,
                o30o0e_body,
                k4wwud_body,
            },
        ))
    }
}
