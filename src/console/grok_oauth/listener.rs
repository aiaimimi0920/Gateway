//! Explicit loopback reception owns no save authority and never changes the registered redirect.
use super::{session::wait_stopped, LoginError, Session, SessionStatus};
use crate::{
    protocol::loopback_callback::{callback, read_head},
    state::AppState,
};
use std::{
    net::{IpAddr, SocketAddr},
    sync::{Arc, Weak},
    time::Duration,
};
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
};

pub(super) fn bind(redirect: &str) -> Result<TcpListener, LoginError> {
    // Authorization has already validated the URI. Numeric addresses avoid localhost/DNS ambiguity.
    let url = url::Url::parse(redirect).map_err(|_| LoginError::Listener)?;
    let ip: IpAddr = match url.host() {
        Some(url::Host::Ipv4(ip)) => ip.into(),
        Some(url::Host::Ipv6(ip)) => ip.into(),
        _ => return Err(LoginError::Listener),
    };
    let port = url
        .port()
        .filter(|port| *port != 0)
        .ok_or(LoginError::Listener)?;
    if !ip.is_loopback() {
        return Err(LoginError::Listener);
    }
    let listener =
        std::net::TcpListener::bind(SocketAddr::new(ip, port)).map_err(|_| LoginError::Listener)?;
    listener
        .set_nonblocking(true)
        .map_err(|_| LoginError::Listener)?;
    TcpListener::from_std(listener).map_err(|_| LoginError::Listener)
}

pub(super) async fn serve(
    listener: TcpListener,
    session: Arc<Session>,
    state: Weak<AppState>,
    client: rquest::Client,
    redirect: String,
) {
    let mut stop = session.cancel.subscribe();
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    loop {
        let accepted = tokio::select! { biased;
            _=wait_stopped(&mut stop)=>break,
            _=tokio::time::sleep_until(session.deadline)=>{ session.view(); break; },
            _=tick.tick()=>{
                if session.view().status != SessionStatus::WaitingUser { break; }
                continue;
            },
            result=listener.accept()=>result,
        };
        let Ok((mut stream, peer)) = accepted else {
            session.cancel();
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
            .and_then(|head| callback(head, &redirect))
        else {
            reply(&mut stream, false).await;
            continue;
        };
        if !state
            .upgrade()
            .is_some_and(|state| super::validate_target(&state, session.binding()).is_ok())
        {
            session.cancel();
            reply(&mut stream, false).await;
            break;
        }
        // Browser disconnect cannot drop an admitted exchange; the registry joins this owner.
        let success = session.complete(&client, &callback).await.is_ok();
        reply(&mut stream, success).await;
        if session.view().status != SessionStatus::WaitingUser {
            break;
        }
    }
}

async fn reply(stream: &mut TcpStream, success: bool) {
    let text = if success {
        "Grok authorization received. Return to Gateway to save the credential."
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
