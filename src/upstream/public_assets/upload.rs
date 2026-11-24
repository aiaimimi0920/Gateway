//! Single credential-free PUT to a vendor-issued URL; never follows redirects or retries.
use super::{checked_url, Failure};
use rquest::header::HeaderMap;
use tokio::time::Instant;

pub(in crate::upstream) async fn put(
    raw: &str,
    headers: HeaderMap,
    body: rquest::Body,
    size: usize,
    deadline: Instant,
) -> Result<(), Failure> {
    let url = checked_url(raw)?;
    tokio::time::timeout_at(deadline, async {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(Failure::Timeout);
        }
        #[cfg(not(test))]
        let client = super::client(&url, remaining).await?;
        #[cfg(test)]
        let client = rquest::Client::builder().no_proxy().build().unwrap();
        #[cfg(test)]
        let url = super::test_target(&url);
        let response = client
            .put(url.as_str())
            .default_headers(false)
            .headers(headers)
            .redirect(rquest::redirect::Policy::none())
            .timeout(remaining)
            .header("content-length", size)
            .body(body)
            .send()
            .await
            .map_err(super::network)?;
        match response.status().as_u16() {
            200 | 201 | 204 => Ok(()),
            status => Err(Failure::Http(status)),
        }
    })
    .await
    .map_err(|_| Failure::Timeout)?
}
