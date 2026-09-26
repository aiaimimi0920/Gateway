use super::*;

impl UpstreamClient {
    pub(in crate::upstream::client) async fn execute_gemini_canvas_text_bootstrap_preflight(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        source_path: &str,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        _origin: &str,
        _authorization: &str,
        timeout: Duration,
    ) -> Result<(), GatewayError> {
        let request =
            gemini_canvas::build_text_bootstrap_preflight_request(bootstrap, source_path)?;
        self.send_gemini_canvas_text_batchexecute_request(
            payload,
            model,
            &request,
            session,
            gemini_canvas::GEMINI_CANVAS_TEXT_BOOTSTRAP_MODEL_HEADER,
            timeout,
        )
        .await
        .map(|_| ())
    }

    pub(in crate::upstream::client) async fn execute_gemini_canvas_text_state_preflight(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        source_path: &str,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        _origin: &str,
        _authorization: &str,
        timeout: Duration,
    ) -> Result<(), GatewayError> {
        let request = gemini_canvas::build_text_state_preflight_request(bootstrap, source_path)?;
        self.send_gemini_canvas_text_batchexecute_request(
            payload,
            model,
            &request,
            session,
            gemini_canvas::GEMINI_CANVAS_TEXT_MODE_SELECTION_MODEL_HEADER,
            timeout,
        )
        .await
        .map(|_| ())
    }

    pub(in crate::upstream::client) async fn execute_gemini_canvas_text_state_variant_preflight(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        source_path: &str,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        state_len: usize,
        tail_index: usize,
        tail_value: Value,
        marker: &str,
        timeout: Duration,
    ) -> Result<(), GatewayError> {
        let request = gemini_canvas::build_text_state_variant_preflight_request(
            bootstrap,
            source_path,
            state_len,
            tail_index,
            tail_value,
            marker,
            gemini_canvas::GEMINI_CANVAS_TEXT_MODE_SELECTION_RPCID,
        )?;
        self.send_gemini_canvas_text_batchexecute_request(
            payload,
            model,
            &request,
            session,
            gemini_canvas::GEMINI_CANVAS_TEXT_MODE_SELECTION_MODEL_HEADER,
            timeout,
        )
        .await
        .map(|_| ())
    }

    pub(in crate::upstream::client) async fn execute_gemini_canvas_text_generic_preflight(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        bootstrap: &gemini_web::GeminiWebBootstrap,
        source_path: &str,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        rpcid: &str,
        rpc_payload: Value,
        model_header: &str,
        timeout: Duration,
    ) -> Result<(), GatewayError> {
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
        .await
        .map(|_| ())
    }

    pub(crate) async fn send_gemini_canvas_text_batchexecute_request(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        request: &gemini_web::GeminiWebRequest,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        model_header: &str,
        timeout: Duration,
    ) -> Result<String, GatewayError> {
        let mut session_clone = session.clone();
        self.send_gemini_canvas_text_batchexecute_request_refreshing_session(
            payload,
            model,
            request,
            &mut session_clone,
            model_header,
            timeout,
        )
        .await
    }
}
