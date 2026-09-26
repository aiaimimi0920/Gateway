use super::*;

impl UpstreamClient {
    pub(in crate::upstream::client) async fn execute_gemini_canvas_media_operation_selection_preflight(
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
        let request = gemini_canvas::build_media_operation_selection_preflight_request(
            bootstrap,
            source_path,
            mode_index,
        )?;
        let model_header =
            gemini_canvas::build_text_batchexecute_model_header(None, batchexecute_header_id);
        self.send_gemini_canvas_text_batchexecute_request(
            payload,
            model,
            &request,
            session,
            &model_header,
            timeout,
        )
        .await
        .map(|_| ())
    }

    pub(in crate::upstream::client) async fn execute_gemini_canvas_media_state_preflight_sequence(
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
        let language = bootstrap.language.clone();
        let primary_mode_index = gemini_canvas::media_primary_ku4jyf_mode_index(mode_index);
        let xhau0b_payload =
            if mode_index == gemini_canvas::GEMINI_CANVAS_STREAM_GENERATE_MUSIC_MODE_INDEX {
                json!([4, [3], 3])
            } else {
                json!([4, [2], 3])
            };
        let model_header =
            gemini_canvas::build_text_batchexecute_model_header(None, batchexecute_header_id);
        let preflights = vec![
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
            ("K4WWud", json!([[1], [language.clone()]])),
            ("CNgdBe", json!([1, [language.clone()], 0])),
            (
                "ku4Jyf",
                json!([
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
                ]),
            ),
            ("CNgdBe", json!([2, [language.clone()], 0, null, [2]])),
            ("ESY5D", json!([[["bard_activity_enabled"]]])),
            ("MaZiqc", json!([13, null, [1, null, 1]])),
            ("MaZiqc", json!([13, null, [0, null, 1]])),
            (
                "ku4Jyf",
                json!([
                    language,
                    null,
                    null,
                    null,
                    4,
                    null,
                    null,
                    [2, 4, 7, 17],
                    null,
                    []
                ]),
            ),
            ("XhaU0b", xhau0b_payload),
        ];

        for (rpcid, rpc_payload) in preflights {
            self.execute_gemini_canvas_text_generic_preflight(
                payload,
                model,
                bootstrap,
                source_path,
                session,
                rpcid,
                rpc_payload,
                &model_header,
                timeout,
            )
            .await?;
        }

        Ok(())
    }
}
