//! Loopback callback arrival grants no management/save authority; wrong state leaves the listener alive.
use super::{session::wait_stopped, Session, SessionStatus};
use crate::{
    protocol::loopback_callback::{callback, read_head},
    state::AppState,
};
use std::{
    sync::{Arc, Weak},
    time::Duration,
};
use tokio::{
    io::AsyncWriteExt,
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
        let Some(callback) = request
            .as_deref()
            .and_then(|head| callback(head, &session.redirect))
        else {
            reply(&mut stream, false).await;
            continue;
        };
        if !state
            .upgrade()
            .is_some_and(|s| super::validate_target(&s, session.binding()).is_ok())
        {
            session.fail_waiting();
            reply(&mut stream, false).await;
            break;
        }
        // The joined owner, not the browser socket, owns the one-shot exchange after admission.
        let success = session.complete(&client, &callback).await.is_ok();
        reply(&mut stream, success).await;
        if session.view().status != SessionStatus::WaitingUser {
            break;
        }
    }
}
async fn reply(stream: &mut TcpStream, success: bool) {
    let text = if success {
        "DIM authorization received. Return to Gateway to save the credential."
    } else {
        "Authorization could not be completed. Return to Gateway for status."
    };
    let response = format!("HTTP/1.1 {}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'none'; frame-ancestors 'none'\r\n\r\n{text}", if success { "200 OK" } else { "400 Bad Request" }, text.len());
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
    fn dim_oauth_management_listener_rejects_host_target_and_request_smuggling() {
        let redirect = "http://127.0.0.1:12345/oauth/callback";
        let valid = "GET /oauth/callback?code=c&state=s HTTP/1.1\r\nHost: 127.0.0.1:12345\r\n\r\n";
        assert_eq!(
            callback(valid.as_bytes(), redirect).as_deref(),
            Some("http://127.0.0.1:12345/oauth/callback?code=c&state=s")
        );
        for invalid in [
            valid.replace("GET", "POST"),
            valid.replace("/oauth/callback?", "/?"),
            valid.replace("/oauth/callback?", "http://evil.invalid/oauth/callback?"),
            valid.replace("127.0.0.1:12345", "evil.invalid"),
            valid.replace("\r\n\r\n", "\r\nHost: 127.0.0.1:12345\r\n\r\n"),
            valid.replace("\r\n\r\n", "\r\nTransfer-Encoding: chunked\r\n\r\n"),
            valid.replace("\r\n\r\n", "\r\nContent-Length: 1\r\n\r\n"),
            format!("{valid}GET / HTTP/1.1\r\n\r\n"),
        ] {
            assert!(callback(invalid.as_bytes(), redirect).is_none());
        }
    }
}
