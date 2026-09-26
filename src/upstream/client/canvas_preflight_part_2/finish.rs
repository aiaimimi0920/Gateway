use super::*;

impl UpstreamClient {
    pub(super) async fn finish_media_capture_parity_preflight(
        &self,
        state: MediaCaptureParityPreflightState,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        source_path: &str,
        session: &mut gemini_canvas::GeminiCanvasPureHttpSession,
        timeout: Duration,
    ) -> Result<GeminiCanvasMediaCaptureParityPreflightResult, GatewayError> {
        let MediaCaptureParityPreflightState {
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
            mut maziqc_probe_body,
            mut maziqc_full_body,
            mut o30o0e_body,
            mut k4wwud_body,
        } = state;
        let middle_initial_requests: Vec<(&str, Value)> = if is_image_mode {
            vec![
                ("K4WWud", json!([[0], [language.clone()]])),
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
                ("sJBwce", json!([[1, 2]])),
                ("DYBcR", json!([language.clone()])),
                ("maGuAc", json!([1])),
                ("Te6DCf", json!([[language.clone()], [1]])),
                ("CNgdBe", json!([1, [language.clone()], 0])),
                ("CNgdBe", json!([2, [language.clone()], 0])),
            ]
        } else {
            vec![
                ("GPRiHf", json!([])),
                ("maGuAc", json!([0])),
                ("maGuAc", json!([1])),
                (
                    "qpEbW",
                    json!([[
                        [1, selection_mode_index],
                        [2, selection_mode_index],
                        [6, selection_mode_index]
                    ]]),
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
            ]
        };

        for (rpcid, rpc_payload) in middle_initial_requests {
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
                    &neutral_header,
                    Some(initial_header2),
                    timeout,
                )
                .await?;
            match rpcid {
                "o30O0e" => {
                    o30o0e_body = Some(body);
                }
                "K4WWud" => {
                    k4wwud_body = Some(body);
                }
                _ => {}
            }
        }

        let media_state_keys =
            gemini_canvas::build_image_state_keys_preflight_request(bootstrap, source_path)?;
        self.send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
            payload,
            model,
            &media_state_keys,
            session,
            &neutral_header,
            Some(selected_header2),
            timeout,
        )
        .await?;

        for (rpcid, rpc_payload) in [
            ("ESY5D", json!([[["bard_activity_enabled"]]])),
            ("MaZiqc", json!([13, null, [1, null, 1]])),
            ("MaZiqc", json!([13, null, [0, null, 1]])),
            ("mhs1xe", json!([[1, 3]])),
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
                    &neutral_header,
                    Some(selected_header2),
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
                _ => {}
            }
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
            &flagged_neutral_header,
            Some(selected_header2),
            timeout,
        )
        .await?;

        let selected_bootstrap =
            gemini_canvas::build_text_bootstrap_preflight_request(bootstrap, source_path)?;
        let selected_bootstrap_body = self
            .send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
                payload,
                model,
                &selected_bootstrap,
                session,
                &flagged_selected_header,
                Some(selected_header2),
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
                popup_zs_visits_cooldown,
                "popup_zs_visits_cooldown",
                gemini_canvas::GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID,
            )?,
            gemini_canvas::build_text_state_variant_preflight_request(
                bootstrap,
                source_path,
                192usize,
                191usize,
                tool_menu_soft_badge_impression_counts,
                "tool_menu_soft_badge_impression_counts",
                gemini_canvas::GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID,
            )?,
        ] {
            self.send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
                payload,
                model,
                &request,
                session,
                &flagged_neutral_header,
                Some(selected_header2),
                timeout,
            )
            .await?;
        }

        let ku4jyf_first = gemini_canvas::build_text_batchexecute_request(
            "ku4Jyf",
            json!([[
                language,
                null,
                null,
                null,
                primary_mode_index,
                null,
                null,
                [32],
                null,
                []
            ]]),
            bootstrap,
            source_path,
        )?;
        self.send_gemini_canvas_text_batchexecute_request_with_header2_refreshing_session(
            payload,
            model,
            &ku4jyf_first,
            session,
            &flagged_neutral_header,
            Some(selected_header2),
            timeout,
        )
        .await?;

        for (rpcid, rpc_payload) in [
            (
                "CNgdBe",
                json!([2, [bootstrap.language.clone()], 0, null, [2]]),
            ),
            ("XhaU0b", xhau0b_payload),
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
                &flagged_neutral_header,
                Some(selected_header2),
                timeout,
            )
            .await?;
        }

        Ok(GeminiCanvasMediaCaptureParityPreflightResult {
            selected_bootstrap_body,
            maziqc_probe_body,
            maziqc_full_body,
            o30o0e_body,
            k4wwud_body,
        })
    }
}
