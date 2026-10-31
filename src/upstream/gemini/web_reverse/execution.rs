use std::collections::HashMap;
use std::pin::Pin;
use std::time::Duration;

use futures::Stream;
use rquest::Client;

use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse};
use crate::protocol::gemini::web_reverse as surface;
use crate::routing::candidate::ProviderAccountPayload;

use super::uploads;

mod exchange;

fn protect(req: &CanonicalRelayRequest, error: GatewayError) -> GatewayError {
    if surface::has_media(req) {
        surface::without_media_replay(error)
    } else {
        error
    }
}

pub type GeminiWebStream = Pin<Box<dyn Stream<Item = Result<bytes::Bytes, rquest::Error>> + Send>>;

pub async fn execute(
    http: &Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let result = async {
        let (body, mut session) = exchange::exchange(
            http,
            timeout,
            payload,
            req,
            model,
            extra_headers,
            uploads::UPLOAD_URL,
        )
        .await?;
        let response = if surface::has_media(req) {
            surface::accumulate_complete_media_response(&body, model)
        } else {
            surface::accumulate_gemini_web_response(&body, model)
        }?;
        session.commit().await;
        Ok(response)
    }
    .await;
    result.map_err(|e| protect(req, e))
}

pub async fn execute_stream(
    http: &Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<GeminiWebStream, GatewayError> {
    let result = async {
        let (body, mut session) = exchange::exchange(
            http,
            timeout,
            payload,
            req,
            model,
            extra_headers,
            uploads::UPLOAD_URL,
        )
        .await?;
        let events = if surface::has_media(req) {
            surface::canonical_to_sse(surface::accumulate_complete_media_response(&body, model)?)
        } else {
            surface::translate_gemini_web_to_openai_sse(&body, model)?
        };
        session.commit().await;
        Ok(Box::pin(futures::stream::iter(events.into_iter().map(Ok))) as GeminiWebStream)
    }
    .await;
    result.map_err(|e| protect(req, e))
}

#[cfg(test)]
#[path = "execution_tests.rs"]
mod tests;
