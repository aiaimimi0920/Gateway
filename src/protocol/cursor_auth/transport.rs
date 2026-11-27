//! Fixed authority, explicit no-redirect policy, bounded bodies and secret-free errors.
use super::AuthError;
use futures::StreamExt;
use std::time::Duration;

pub(crate) fn client() -> Result<rquest::Client, AuthError> {
    let builder = crate::http_client::builder();
    #[cfg(test)]
    let builder = if TEST_ORIGIN.try_with(|_| ()).is_ok() {
        builder.no_proxy()
    } else {
        builder
    };
    builder
        .redirect(rquest::redirect::Policy::none())
        .timeout(Duration::from_secs(15))
        .user_agent("Neuro-Gateway-Cursor-Import/0.1")
        .build()
        .map_err(|_| AuthError("cursor_transport_unavailable"))
}
#[cfg(test)]
tokio::task_local! { pub(crate) static TEST_ORIGIN: String; }
pub(super) fn target(path: &str) -> url::Url {
    #[cfg(test)]
    if let Ok(origin) = TEST_ORIGIN.try_with(Clone::clone) {
        return url::Url::parse(&format!("{origin}{path}")).unwrap();
    }
    url::Url::parse(&format!("https://api2.cursor.sh{path}")).unwrap()
}
pub(super) async fn read(request: rquest::RequestBuilder) -> Result<(u16, Vec<u8>), AuthError> {
    let response = request
        .header("accept", "application/json")
        .send()
        .await
        .map_err(|_| AuthError("cursor_transport_failed"))?;
    let status = response.status().as_u16();
    if (300..400).contains(&status) {
        return Err(AuthError("cursor_redirect_rejected"));
    }
    if status != 200 {
        return Ok((status, Vec::new()));
    }
    if response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .is_none_or(|v| {
            !v.split(';')
                .next()
                .unwrap_or("")
                .trim()
                .eq_ignore_ascii_case("application/json")
        })
    {
        return Err(AuthError("cursor_invalid_content_type"));
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| AuthError("cursor_transport_failed"))?;
        if bytes.len() + chunk.len() > 65536 {
            return Err(AuthError("cursor_response_too_large"));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok((status, bytes))
}
