//! Bound each parser poll: multer otherwise drains all ready input before returning a field.
use axum::{body::Body, extract::Request};
use bytes::Bytes;
use futures::StreamExt;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[derive(Clone)]
pub(super) struct Budget(Arc<AtomicUsize>);

impl Budget {
    pub(super) fn file(&self, size: usize) {
        self.0.store(size + 16384, Ordering::Release);
    }
}

pub(super) fn bounded(request: Request) -> (Request, Budget) {
    // Metadata and MIME headers must fit before the validated file size opens the budget.
    let budget = Budget(Arc::new(AtomicUsize::new(16384)));
    let limit = budget.clone();
    let request = request.map(|body| {
        let stream = futures::stream::unfold(
            (body.into_data_stream(), Bytes::new(), 0usize),
            move |(mut source, mut pending, mut read)| {
                let limit = limit.clone();
                async move {
                    // This Pending prevents multer's eager poll loop from buffering the whole file.
                    tokio::task::yield_now().await;
                    if pending.is_empty() {
                        match source.next().await? {
                            Ok(bytes) => pending = bytes,
                            Err(_) => {
                                return Some((
                                    Err(std::io::Error::other("Upload input failed")),
                                    (source, pending, read),
                                ));
                            }
                        }
                    }
                    let allowed = limit.0.load(Ordering::Acquire);
                    let remaining = allowed.saturating_sub(read);
                    if remaining == 0 {
                        return Some((
                            Err(std::io::Error::other("Upload MIME headers exceed limit")),
                            (source, pending, read),
                        ));
                    }
                    // Keep room for the parser's next poll while metadata and file headers are admitted.
                    let chunk_size = if allowed == 16384 { 1024 } else { 64 * 1024 };
                    let bytes = pending.split_to(pending.len().min(chunk_size).min(remaining));
                    read += bytes.len();
                    Some((Ok(bytes), (source, pending, read)))
                }
            },
        );
        Body::from_stream(stream)
    });
    (request, budget)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::{FromRequest, Multipart};

    #[tokio::test]
    async fn luma_files_reject_unbounded_multipart_headers_before_file_admission() {
        let request = Request::builder()
            .header("content-type", "multipart/form-data; boundary=b")
            .body(Body::from(format!("--b\r\nX-Large: {}", "x".repeat(32768))))
            .unwrap();
        let (request, _) = bounded(request);
        let mut form = Multipart::from_request(request, &()).await.unwrap();
        assert!(form.next_field().await.is_err());
    }
}
