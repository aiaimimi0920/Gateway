//! A single bounded connection per session; callback arrival never grants management/save authority.
use super::{session::wait_stopped, Session};
use crate::state::AppState;
use std::{
    sync::{Arc, Weak},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

pub(super) async fn serve(
    listener: TcpListener,
    session: Arc<Session>,
    state: Weak<AppState>,
    client: rquest::Client,
) {
    let mut stop = session.stop.subscribe();
    loop {
        let accepted = tokio::select! { biased;
            _=wait_stopped(&mut stop)=>break,
            _=tokio::time::sleep_until(session.deadline)=>{ session.view(); break; },
            result=listener.accept()=>result,
        };
        let Ok((mut stream, peer)) = accepted else {
            session.fail_waiting();
            break;
        };
        if !peer.ip().is_loopback() {
            continue;
        }
        let request = tokio::select! { biased;
            _=wait_stopped(&mut stop)=>break,
            _=tokio::time::sleep_until(session.deadline)=>{ session.view(); break; },
            result=tokio::time::timeout(Duration::from_secs(5), read_head(&mut stream))=>result.ok().and_then(Result::ok),
        };
        let callback = request
            .as_deref()
            .and_then(|head| callback(head, &session.redirect));
        let Some(callback) = callback else {
            reply(&mut stream, false).await;
            continue;
        };
        let valid = state.upgrade().is_some_and(|s| {
            super::validate_target(&s, session.binding(), &session.target).is_ok()
        });
        if !valid {
            session.fail_waiting();
            reply(&mut stream, false).await;
            break;
        }
        // Owned by the joined listener, not the browser connection: disconnect cannot resend a code.
        let success = session.complete(&client, &callback).await.is_ok();
        reply(&mut stream, success).await;
        break;
    }
}
async fn read_head(stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
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
fn callback(head: &[u8], redirect: &str) -> Option<String> {
    let head = std::str::from_utf8(head).ok()?.strip_suffix("\r\n\r\n")?;
    let mut lines = head.split("\r\n");
    let request: Vec<_> = lines.next()?.split(' ').collect();
    let ["GET", uri, "HTTP/1.1"] = request.as_slice() else {
        return None;
    };
    if uri.len() > 16_384 || !uri.starts_with("/?") || uri.contains('#') {
        return None;
    }
    let expected_host = redirect.strip_prefix("http://")?.strip_suffix('/')?;
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
    Some(format!("{redirect}{}", &uri[1..]))
}
async fn reply(stream: &mut TcpStream, success: bool) {
    let text = if success {
        "Vertex authorization received. Return to Gateway to save the credential."
    } else {
        "Authorization could not be completed. Return to Gateway for status."
    };
    let response = format!("HTTP/1.1 {}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'none'; frame-ancestors 'none'\r\n\r\n{text}", if success { "200 OK" } else { "400 Bad Request" }, text.len());
    // Shutdown must not wait forever for a peer that never reads its callback response.
    let _ = tokio::time::timeout(
        Duration::from_secs(1),
        stream.write_all(response.as_bytes()),
    )
    .await;
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vertex_oauth_management_listener_rejects_request_target_host_and_framing_confusion() {
        let redirect = "http://127.0.0.1:12345/";
        let valid = "GET /?code=c&state=s HTTP/1.1\r\nHost: 127.0.0.1:12345\r\n\r\n";
        assert_eq!(
            callback(valid.as_bytes(), redirect).as_deref(),
            Some("http://127.0.0.1:12345/?code=c&state=s")
        );
        for invalid in [
            valid.replace("GET", "POST"),
            valid.replace("/?code", "http://evil.invalid/?code"),
            valid.replace("127.0.0.1:12345", "evil.invalid"),
            valid.replace("\r\n\r\n", "\r\nHost: 127.0.0.1:12345\r\n\r\n"),
            valid.replace("\r\n\r\n", "\r\nTransfer-Encoding: chunked\r\n\r\n"),
            format!("{valid}GET / HTTP/1.1\r\n\r\n"),
        ] {
            assert!(callback(invalid.as_bytes(), redirect).is_none());
        }
    }
}
