//! Preserve an already bounded JSON payload without changing other public extractors.
use crate::{error::GatewayError, protocol::codex_wire::NativeRequestWire};
use async_trait::async_trait;
use axum::{
    body::Bytes,
    extract::{FromRequest, Request},
    http::header::CONTENT_TYPE,
};

#[derive(Clone, Copy)]
pub(crate) struct ClientRequestId(pub bool);

pub struct NativeJson {
    pub value: serde_json::Value,
    pub wire: NativeRequestWire,
}

#[async_trait]
impl<S: Send + Sync> FromRequest<S> for NativeJson {
    type Rejection = GatewayError;
    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let content_type = req
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let mime = content_type
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        if mime != "application/json"
            && !(mime.starts_with("application/") && mime.ends_with("+json"))
        {
            return Err(GatewayError::bad_request(
                "Invalid JSON body: expected application/json",
            ));
        }
        let query = req.uri().query().map(str::to_owned);
        if query.as_ref().is_some_and(|q| q.len() > 8192) {
            return Err(GatewayError::bad_request("Request query too large"));
        }
        let client_request_id = req
            .extensions()
            .get::<ClientRequestId>()
            .map(|v| v.0)
            .unwrap_or_else(|| req.headers().contains_key("x-request-id"));
        let header_order = req
            .headers()
            .keys()
            .filter(|name| {
                crate::protocol::codex_wire::FORWARDED_HEADERS.contains(&name.as_str())
                    || crate::protocol::claude_wire::HEADERS.contains(&name.as_str())
                    || matches!(
                        name.as_str(),
                        "authorization"
                            | "content-type"
                            | "x-api-key"
                            | "api-key"
                            | "x-goog-api-key"
                            | "traceparent"
                            | "tracestate"
                            | "x-request-id"
                    )
            })
            .map(|name| name.as_str().to_owned())
            .collect();
        let connection = req
            .extensions()
            .get::<crate::http_client::NativeCodexConnection>()
            .cloned();
        // Bytes uses axum's configured body limit and the outer RequestBodyLimitLayer.
        let body = Bytes::from_request(req, state).await.map_err(|rejection| {
            let status = rejection.status().as_u16();
            let mut error = GatewayError::bad_request(if status == 413 {
                "Request body too large".to_owned()
            } else {
                format!("Invalid JSON body: {}", rejection.body_text())
            });
            if status == 413 {
                error.http_status = Some(413);
                error.code = Some("request_too_large".into());
            }
            error
        })?;
        let value = super::native_json_decode::decode(&body)
            .map_err(|e| GatewayError::bad_request(format!("Invalid JSON body: {e}")))?;
        Ok(Self {
            value,
            wire: NativeRequestWire {
                native_session: None,
                compact: false,
                websocket: None,
                connection,
                body,
                query,
                client_request_id,
                header_order,
            },
        })
    }
}

#[cfg(test)]
#[path = "native_json_tests.rs"]
mod tests;
