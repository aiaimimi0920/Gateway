//! Bounded HTTP parsing shared by local authorization listeners; no redirect or secret echo.
use tokio::{io::AsyncReadExt, net::TcpStream};

pub(crate) async fn read_head(stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut chunk = [0; 2048];
    loop {
        let n = stream.read(&mut chunk).await?;
        if n == 0 || bytes.len() + n > 32 * 1024 {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        bytes.extend_from_slice(&chunk[..n]);
        if bytes.windows(4).any(|w| w == b"\r\n\r\n") {
            return Ok(bytes);
        }
    }
}
pub(crate) fn callback(head: &[u8], redirect: &str) -> Option<String> {
    let head = std::str::from_utf8(head).ok()?.strip_suffix("\r\n\r\n")?;
    let mut lines = head.split("\r\n");
    let request: Vec<_> = lines.next()?.split(' ').collect();
    let ["GET", uri, "HTTP/1.1"] = request.as_slice() else {
        return None;
    };
    let target = url::Url::parse(redirect).ok()?;
    let prefix = format!("{}?", target.path());
    if uri.len() > 16_384 || !uri.starts_with(&prefix) || uri.contains('#') {
        return None;
    }
    let expected_host = redirect
        .strip_prefix("http://")?
        .strip_suffix(target.path())?;
    let mut host = None;
    for (i, line) in lines.enumerate() {
        if i >= 64 {
            return None;
        }
        let (key, value) = line.split_once(':')?;
        let key = axum::http::HeaderName::from_bytes(key.as_bytes()).ok()?;
        let value = value.trim_matches([' ', '\t']);
        axum::http::HeaderValue::from_str(value).ok()?;
        if key == "host" {
            if host.replace(value).is_some() {
                return None;
            }
        } else if key == "transfer-encoding" || key == "content-length" && value != "0" {
            return None;
        }
    }
    if host != Some(expected_host) {
        return None;
    }
    Some(format!("http://{expected_host}{uri}"))
}
