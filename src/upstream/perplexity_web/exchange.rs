//! Buffered and live consumers borrow the same one-send, strict-decoder and total-deadline owner.
use super::{transport, Session, MAX_DEADLINE};
use crate::{
    error::GatewayError,
    protocol::perplexity_web::{input_failure, Decoder, Update},
};
use bytes::Bytes;
use futures::{Stream, StreamExt};
use std::{pin::Pin, time::Duration};
use tokio::time::Instant;

pub(super) struct Exchange {
    stream: Pin<Box<dyn Stream<Item = Result<Bytes, rquest::Error>> + Send>>,
    decoder: Decoder,
    deadline: Instant,
}
impl Exchange {
    pub(super) async fn open(
        client: &rquest::Client,
        session: &Session,
        body: &serde_json::Value,
        timeout: Duration,
        url: &str,
    ) -> Result<Self, GatewayError> {
        if timeout.is_zero() || timeout > MAX_DEADLINE {
            return Err(input_failure(
                "perplexity_web_deadline_invalid",
                "Perplexity Web deadline 必须在 0 到 120 秒之间。",
            ));
        }
        let deadline = Instant::now() + timeout;
        // Test-only remapping is completed before returning a lazy stream; no production override.
        #[cfg(test)]
        let local_url = (url == super::ASK_URL).then(|| super::test_support::target(url));
        #[cfg(test)]
        let url = local_url.as_deref().unwrap_or(url);
        let response = tokio::time::timeout_at(
            deadline,
            client
                .post(url)
                .default_headers(false)
                .headers(session.headers())
                .redirect(rquest::redirect::Policy::none())
                .timeout(timeout)
                .json(body)
                .send(),
        )
        .await
        .map_err(|_| transport::timeout_error())?
        .map_err(transport::io_error)?;
        transport::admit_response(&response)?;
        Ok(Self {
            stream: Box::pin(response.bytes_stream()),
            decoder: Decoder::new(),
            deadline,
        })
    }
    pub(super) async fn next(&mut self) -> Result<Update, GatewayError> {
        tokio::time::timeout_at(self.deadline, self.read())
            .await
            .map_err(|_| transport::timeout_error())?
    }
    async fn read(&mut self) -> Result<Update, GatewayError> {
        loop {
            if let Some(event) = self.decoder.next_event()? {
                return Ok(event);
            }
            let Some(bytes) = self.stream.next().await else {
                self.decoder.finish()?;
                return Err(crate::protocol::perplexity_web::failure(
                    "perplexity_web_incomplete",
                    "Perplexity Web 未交付完整答案。",
                ));
            };
            self.decoder
                .push_chunk(bytes.map_err(transport::io_error)?)?;
        }
    }
}
