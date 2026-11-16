//! The consumer drives the original paid future and its bounded mailbox; there is no producer task.
use super::*;
use futures::Stream;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
pub(super) struct Delivery {
    pub bytes: Bytes,
    pub terminal: bool,
}
pub(super) struct Source {
    future: Option<Pin<Box<dyn Future<Output = Result<(), GatewayError>> + Send>>>,
    receiver: tokio::sync::mpsc::Receiver<Delivery>,
    result: Option<Result<(), GatewayError>>,
    prefetched: Option<Delivery>,
    terminal: Arc<std::sync::atomic::AtomicBool>,
}
impl Source {
    pub fn new(
        future: impl Future<Output = Result<(), GatewayError>> + Send + 'static,
        receiver: tokio::sync::mpsc::Receiver<Delivery>,
        terminal: Arc<std::sync::atomic::AtomicBool>,
    ) -> Self {
        Self {
            future: Some(Box::pin(future)),
            receiver,
            result: None,
            prefetched: None,
            terminal,
        }
    }
    pub async fn prefetch(&mut self) -> Result<(), GatewayError> {
        // Preflight is not public delivery: a buffered JSON terminal must not count as consumed yet.
        let first = std::future::poll_fn(|cx| self.poll_delivery(cx))
            .await
            .ok_or_else(|| business(BusinessError::InvalidResponse))??;
        self.prefetched = Some(first);
        Ok(())
    }
    fn poll_delivery(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Delivery, GatewayError>>> {
        if let Some(future) = &mut self.future {
            if let Poll::Ready(result) = future.as_mut().poll(cx) {
                self.result = Some(result);
                self.future = None;
            }
        }
        match self.receiver.poll_recv(cx) {
            Poll::Ready(Some(delivery)) => Poll::Ready(Some(Ok(delivery))),
            Poll::Ready(None) => Poll::Ready(match self.result.take() {
                Some(Err(error)) => Some(Err(error)),
                _ => None,
            }),
            Poll::Pending => Poll::Pending,
        }
    }
}
impl Stream for Source {
    type Item = Result<Bytes, StreamError<rquest::Error>>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        let next = match this.prefetched.take() {
            Some(delivery) => Poll::Ready(Some(Ok(delivery))),
            None => this.poll_delivery(cx),
        };
        next.map(|item| {
            item.map(|result| {
                result
                    .map(|delivery| {
                        if delivery.terminal {
                            this.terminal
                                .store(true, std::sync::atomic::Ordering::Release);
                        }
                        delivery.bytes
                    })
                    .map_err(|error| {
                        crate::protocol::stream_error::ProtocolStreamError::classified(
                            "FreeBuff original stream did not complete",
                            error,
                        )
                        .into()
                    })
            })
        })
    }
}
#[cfg(test)]
mod tests;
