//! Resolve an accepted Web image without sending another generation.
use super::*;
use crate::protocol::gemini_canvas::image_recovery::{
    accepted_image_turn, image_history_request, image_read_error, recovered_image_assets,
};
use crate::upstream::gemini::web_reverse::SessionCookies;

#[path = "image_recovery/http.rs"]
mod http;

impl UpstreamClient {
    pub(in crate::upstream::client) async fn resolve_gemini_web_image_assets(
        &self,
        payload: &ProviderAccountPayload,
        session: &gemini_canvas::GeminiCanvasPureHttpSession,
        cookies: Option<SessionCookies>,
        primary: &str,
        timeout: Duration,
    ) -> Result<Vec<gemini_canvas::GeminiCanvasMediaAsset>, GatewayError> {
        match gemini_canvas::extract_stream_generate_media_assets(
            primary,
            gemini_canvas::GeminiCanvasMediaOperation::Image,
        ) {
            Ok(assets) => return Ok(assets),
            Err(error)
                if error.code.as_deref() == Some("gemini_canvas_image_generation_unavailable") =>
            {
                return Err(image_read_error(
                    "gemini_canvas_image_generation_unavailable",
                ));
            }
            Err(_) => {}
        }
        // Binding precedes all I/O. No raw text IDs, latest-turn search or prompt fallback.
        let turn = accepted_image_turn(primary)?;
        if timeout.is_zero() {
            return Err(image_read_error("gemini_web_image_read_timeout"));
        }
        let work = async {
            let headers = http::read_headers(payload, session)?;
            let mut cookies = match cookies {
                Some(cookies) => cookies,
                None => SessionCookies::new(&payload.base_url, &headers)
                    .map_err(|_| image_read_error("gemini_web_image_read_cookie_invalid"))?,
            };
            let bootstrap = http::bootstrap(self, payload, &headers, &mut cookies).await?;
            for attempt in 0..6 {
                if attempt > 0 {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
                let request = image_history_request(&turn, &bootstrap)?;
                let body = http::history(self, payload, &headers, &request, &mut cookies).await?;
                if let Some(assets) = recovered_image_assets(&body, &turn)? {
                    return Ok(assets);
                }
            }
            Err(image_read_error("gemini_web_image_read_pending"))
        };
        // The deadline includes bootstrap, network/body reads and sleeps. Dropping the
        // caller drops this future; no detached poller or persisted private history remains.
        tokio::time::timeout(timeout.min(Duration::from_secs(30)), work)
            .await
            .map_err(|_| image_read_error("gemini_web_image_read_timeout"))?
    }
}
