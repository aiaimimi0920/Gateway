//! Bridge multipart to a storage body with backpressure and no detached task or disk spool.
use crate::{error::GatewayError, protocol::luma_agents::file_upload::UploadBody};
use axum::extract::Multipart;
use futures::{channel::mpsc, SinkExt, StreamExt};
use std::time::Duration;

pub(super) fn body(mut form: Multipart, size: usize, budget: super::ingress::Budget) -> UploadBody {
    // At most one queued chunk plus the producer's current chunk, independent of file size.
    let (mut sender, receiver) = mpsc::channel::<bytes::Bytes>(1);
    let completion = Box::pin(async move {
        let invalid = || GatewayError::bad_request("Invalid or interrupted Luma upload stream.");
        let mut field = tokio::time::timeout(Duration::from_secs(60), form.next_field())
            .await
            .map_err(|_| invalid())?
            .map_err(|_| invalid())?
            .ok_or_else(invalid)?;
        if field.name() != Some("file") {
            return Err(invalid());
        }
        budget.file(size);
        let mut received = 0usize;
        loop {
            let chunk = tokio::time::timeout(Duration::from_secs(60), field.chunk())
                .await
                .map_err(|_| invalid())?
                .map_err(|_| invalid())?;
            let Some(chunk) = chunk else { break };
            if chunk.len() > size - received {
                return Err(invalid());
            }
            received += chunk.len();
            sender.send(chunk).await.map_err(|_| invalid())?;
        }
        drop(field);
        if received != size
            || tokio::time::timeout(Duration::from_secs(60), form.next_field())
                .await
                .map_err(|_| invalid())?
                .map_err(|_| invalid())?
                .is_some()
        {
            return Err(invalid());
        }
        Ok(())
    });
    UploadBody {
        body: rquest::Body::wrap_stream(receiver.map(Ok::<_, std::io::Error>)),
        completion,
        size,
    }
}

#[cfg(test)]
mod tests;
