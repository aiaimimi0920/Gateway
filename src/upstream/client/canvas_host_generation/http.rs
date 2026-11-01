//! HTTP-only Canvas host transport: one account, one POST, no worker or browser fallback.
use super::*;
use crate::upstream::gemini::web_reverse::SessionCookies;

#[path = "http_session.rs"]
mod session;
#[path = "http_wire.rs"]
mod wire;

const PROVIDER: &str = "gemini_canvas_web_reverse_compatible";
const MAX_RESPONSE_BYTES: usize = 32 * 1024 * 1024;

fn remaining(end: tokio::time::Instant) -> Result<Duration, GatewayError> {
    end.checked_duration_since(tokio::time::Instant::now())
        .filter(|time| !time.is_zero())
        .ok_or_else(|| failure("gemini_canvas_http_deadline_exceeded"))
}

fn failure(code: &str) -> GatewayError {
    GatewayError::service_unavailable(
        "Canvas HTTP host request failed; private upstream details were omitted.",
    )
    .with_provider(PROVIDER)
    .with_code(code)
}

async fn bounded_body(response: rquest::Response, limit: usize) -> Result<String, GatewayError> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(failure("gemini_canvas_http_response_too_large"));
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| failure("gemini_canvas_http_network_failed"))?;
        if bytes.len().saturating_add(chunk.len()) > limit {
            return Err(failure("gemini_canvas_http_response_too_large"));
        }
        bytes
            .try_reserve(chunk.len())
            .map_err(|_| failure("gemini_canvas_http_response_too_large"))?;
        bytes.extend_from_slice(&chunk);
    }
    String::from_utf8(bytes).map_err(|_| failure("gemini_canvas_http_response_invalid"))
}

fn admit_response(status: u16, content_type: Option<&str>, body: &str) -> Result<(), GatewayError> {
    if gemini_web::response_indicates_browser_challenge(status, content_type, body) {
        return Err(failure("gemini_canvas_http_challenge_requires_user"));
    }
    match status {
        200 => Ok(()),
        401 => Err(wire::rpc_error(16)),
        403 => Err(wire::rpc_error(7)),
        429 => Err(wire::rpc_error(8)),
        300..=399 => Err(failure("gemini_canvas_http_redirect_rejected")),
        _ => Err(failure("gemini_canvas_http_upstream_failed")),
    }
}

fn headers(payload: &ProviderAccountPayload) -> HeaderMap {
    let mut result = HeaderMap::new();
    insert_header_map_value(
        &mut result,
        "user-agent",
        gemini_web::GEMINI_WEB_DEFAULT_USER_AGENT,
    );
    insert_header_map_value(&mut result, "accept", "*/*");
    insert_header_map_value(
        &mut result,
        "accept-language",
        &gemini_canvas::locale_from_payload(payload),
    );
    result
}

impl UpstreamClient {
    pub(in crate::upstream::client) async fn execute_gemini_canvas_http_host_generate(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        body: &Value,
        deadline: Duration,
    ) -> Result<Value, GatewayError> {
        // All admission completes before network I/O, including the supported model and JSON size.
        let origin = session::origin(payload)?;
        let auth_user = session::auth_user(payload)?;
        let end = tokio::time::Instant::now() + deadline;
        tokio::time::timeout_at(end, async {
            let packed = wire::form_body(model, body)?;
            remaining(end)?;
            let mut cookies = session::load(payload, &origin).await?;
            self.exchange_canvas_http(
                payload,
                model,
                &origin,
                &auth_user,
                &mut cookies,
                packed,
                end,
            )
            .await
        })
        .await
        .map_err(|_| failure("gemini_canvas_http_deadline_exceeded"))?
    }

    async fn exchange_canvas_http(
        &self,
        payload: &ProviderAccountPayload,
        model: &str,
        origin: &str,
        auth_user: &str,
        cookies: &mut SessionCookies,
        packed: String,
        end: tokio::time::Instant,
    ) -> Result<Value, GatewayError> {
        let app_path = if auth_user == "0" {
            "/app".to_string()
        } else {
            format!("/u/{auth_user}/app")
        };
        let app_url = format!("{origin}{app_path}");
        let rpc_url = format!("{origin}/_/BardChatUi/data/batchexecute");
        // Missing signing material fails closed before even bootstrapping a guest session.
        session::authorize(&mut HeaderMap::new(), cookies, &rpc_url, origin, auth_user)?;
        let mut bootstrap_headers = headers(payload);
        cookies
            .apply(&app_url, &mut bootstrap_headers)
            .map_err(|_| failure("gemini_canvas_http_state_invalid"))?;
        let response = self
            .http
            .get(&app_url)
            .headers(bootstrap_headers)
            .query(&[("hl", "en")])
            .redirect(rquest::redirect::Policy::none())
            .timeout(remaining(end)?)
            .send()
            .await
            .map_err(|_| failure("gemini_canvas_http_network_failed"))?;
        let status = response.status().as_u16();
        let response_headers = response.headers().clone();
        let content_type = response_headers
            .get(rquest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok());
        let html = bounded_body(response, 8 * 1024 * 1024).await?;
        admit_response(status, content_type, &html)?;
        let bootstrap = gemini_web::parse_bootstrap_from_app_html(&html, Some("en"))
            .map_err(|_| failure("gemini_canvas_http_bootstrap_invalid"))?;
        let valid = |value: &Option<String>| {
            value
                .as_ref()
                .is_some_and(|v| !v.is_empty() && v.len() <= 2048)
        };
        if !valid(&bootstrap.build_label) || !valid(&bootstrap.session_id) {
            return Err(failure("gemini_canvas_http_bootstrap_incomplete"));
        }
        // A missing SNlM0e is permitted: this RPC also accepts the verified signed Cookie route.
        cookies.receive(&app_url, &response_headers);
        let mut headers = headers(payload);
        session::authorize(&mut headers, cookies, &rpc_url, origin, auth_user)?;
        insert_header_map_value(&mut headers, "referer", &app_url);
        let query = [
            ("rpcids", wire::RPC),
            ("source-path", app_path.as_str()),
            ("bl", bootstrap.build_label.as_deref().unwrap()),
            ("f.sid", bootstrap.session_id.as_deref().unwrap()),
            ("hl", "en"),
            ("authuser", auth_user),
            ("rt", "c"),
        ];
        let form = wire::form(&packed, bootstrap.access_token.as_deref())?;
        // Exactly one generation send. No XSRF retry, alternate endpoint, Web or browser fallback.
        let response = self
            .http
            .post(&rpc_url)
            .headers(headers)
            .query(&query)
            .header(
                rquest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(form)
            .redirect(rquest::redirect::Policy::none())
            .timeout(remaining(end)?)
            .send()
            .await
            .map_err(|_| failure("gemini_canvas_http_network_failed"))?;
        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get(rquest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let text = bounded_body(response, MAX_RESPONSE_BYTES).await?;
        admit_response(status, content_type.as_deref(), &text)?;
        let generated = wire::parse_response(&text)?;
        debug!(
            model,
            status,
            execution_surface = "canvas_http_host",
            "Canvas HTTP host generation completed"
        );
        Ok(generated)
    }
}
