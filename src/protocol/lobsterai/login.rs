//! The caller owns the socket and future: dropping either releases the listener with no detached task.
use super::{Authorization, Client, CodeGrant, LoginError, ReceivedSession};
use crate::protocol::loopback_callback::{callback, read_head};
use std::time::Duration;
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
};

pub struct PendingLogin {
    listener: TcpListener,
    authorization: Authorization,
}
impl PendingLogin {
    pub async fn bind(client: Client) -> Result<Self, LoginError> {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(|_| LoginError::Listener)?;
        let port = listener
            .local_addr()
            .map_err(|_| LoginError::Listener)?
            .port();
        let authorization =
            Authorization::new(client, &format!("http://127.0.0.1:{port}/auth/callback"))?;
        Ok(Self {
            listener,
            authorization,
        })
    }
    pub fn authorization_url(&self) -> String {
        self.authorization.authorization_url()
    }
    pub fn redirect_uri(&self) -> &str {
        self.authorization.redirect_uri()
    }
    pub async fn receive(self) -> Result<CodeGrant, LoginError> {
        tokio::time::timeout_at(self.authorization.deadline, async move {
            loop {
                let (mut stream, peer) = self
                    .listener
                    .accept()
                    .await
                    .map_err(|_| LoginError::Listener)?;
                if !peer.ip().is_loopback() {
                    continue;
                }
                let head = tokio::time::timeout(Duration::from_secs(5), read_head(&mut stream))
                    .await
                    .ok()
                    .and_then(Result::ok);
                let candidate = head
                    .as_deref()
                    .and_then(|head| callback(head, self.redirect_uri()));
                let accepted = candidate
                    .as_deref()
                    .is_some_and(|callback| self.authorization.callback_code(callback).is_ok());
                reply(&mut stream, accepted).await;
                if accepted {
                    // Socket acknowledgement does not claim a token exchange or durable save succeeded.
                    return self
                        .authorization
                        .into_code_grant(candidate.as_deref().expect("validated callback"));
                }
            }
        })
        .await
        .map_err(|_| LoginError::Expired)?
    }
    pub async fn complete(self) -> Result<ReceivedSession, LoginError> {
        self.receive().await?.exchange().await
    }
    #[cfg(test)]
    pub(super) fn expire_in(&mut self, duration: Duration) {
        self.authorization.deadline = tokio::time::Instant::now() + duration;
    }
}
async fn reply(stream: &mut TcpStream, accepted: bool) {
    let (status, text) = if accepted {
        ("200 OK", "授权回调已接收，请返回 Gateway 查看结果。")
    } else {
        ("400 Bad Request", "授权回调无效，请返回 Gateway。")
    };
    let response = format!("HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'none'; frame-ancestors 'none'\r\n\r\n{text}", text.len());
    let _ = tokio::time::timeout(
        Duration::from_secs(1),
        stream.write_all(response.as_bytes()),
    )
    .await;
}
impl std::fmt::Debug for PendingLogin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LobsterPendingLogin { callback: <redacted> }")
    }
}
