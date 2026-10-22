//! Bounded proxy handshakes. Authentication is consumed here, never by origin HTTP.
use crate::{dial::Io, Error, Result};
use http::{HeaderValue, Uri};
use std::net::IpAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub(crate) async fn connect(io: &mut Io, target: &Uri, auth: Option<&HeaderValue>) -> Result<()> {
    let host = target.host().ok_or(Error::Target)?;
    let authority = format!("{}:{}", host, target.port_u16().unwrap_or(443));
    let mut head = format!(
        "CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\nUser-Agent: Go-http-client/1.1\r\n"
    );
    if let Some(auth) = auth {
        head.push_str("Proxy-Authorization: ");
        head.push_str(auth.to_str().map_err(|_| Error::UnsupportedProxy)?);
        head.push_str("\r\n");
    }
    head.push_str("\r\n");
    io.write_all(head.as_bytes())
        .await
        .map_err(|_| Error::Network)?;
    let mut response = Vec::with_capacity(1024);
    while !response.ends_with(b"\r\n\r\n") {
        if response.len() >= 16384 {
            return Err(Error::Network);
        }
        response.push(io.read_u8().await.map_err(|_| Error::Network)?);
    }
    let line = response
        .split(|c| *c == b'\n')
        .next()
        .ok_or(Error::Network)?;
    let mut fields = line.trim_ascii().split(|c| *c == b' ');
    if !matches!(fields.next(), Some(b"HTTP/1.1" | b"HTTP/1.0")) || fields.next() != Some(b"200") {
        return Err(Error::Network);
    }
    Ok(())
}

pub(crate) async fn socks5(io: &mut Io, target: &Uri, auth: Option<(&str, &str)>) -> Result<()> {
    // Like Go's net/http, both socks5 and socks5h resolve domain names at the proxy.
    if auth
        .is_some_and(|(user, password)| user.is_empty() || user.len() > 255 || password.len() > 255)
    {
        return Err(Error::UnsupportedProxy);
    }
    io.write_all(if auth.is_some() {
        &[5, 2, 0, 2]
    } else {
        &[5, 1, 0]
    })
    .await
    .map_err(|_| Error::Network)?;
    let mut reply = [0u8; 2];
    io.read_exact(&mut reply)
        .await
        .map_err(|_| Error::Network)?;
    if reply[0] != 5 {
        return Err(Error::Network);
    }
    match reply[1] {
        0 => {}
        2 => {
            let (user, password) = auth.ok_or(Error::UnsupportedProxy)?;
            let mut message = vec![1, user.len() as u8];
            message.extend_from_slice(user.as_bytes());
            message.push(password.len() as u8);
            message.extend_from_slice(password.as_bytes());
            io.write_all(&message).await.map_err(|_| Error::Network)?;
            io.read_exact(&mut reply)
                .await
                .map_err(|_| Error::Network)?;
            if reply != [1, 0] {
                return Err(Error::Network);
            }
        }
        _ => return Err(Error::UnsupportedProxy),
    }
    let host = target.host().ok_or(Error::Target)?.trim_matches(['[', ']']);
    let mut message = vec![5, 1, 0];
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => {
            message.push(1);
            message.extend_from_slice(&ip.octets());
        }
        Ok(IpAddr::V6(ip)) => {
            message.push(4);
            message.extend_from_slice(&ip.octets());
        }
        Err(_) => {
            if host.is_empty() || host.len() > 255 {
                return Err(Error::Target);
            }
            message.extend_from_slice(&[3, host.len() as u8]);
            message.extend_from_slice(host.as_bytes());
        }
    }
    let port = target
        .port_u16()
        .unwrap_or(if target.scheme_str() == Some("https") {
            443
        } else {
            80
        });
    message.extend_from_slice(&port.to_be_bytes());
    io.write_all(&message).await.map_err(|_| Error::Network)?;
    let mut head = [0u8; 4];
    io.read_exact(&mut head).await.map_err(|_| Error::Network)?;
    if head[..3] != [5, 0, 0] {
        return Err(Error::Network);
    }
    let length = match head[3] {
        1 => 4,
        4 => 16,
        3 => io.read_u8().await.map_err(|_| Error::Network)? as usize,
        _ => return Err(Error::Network),
    };
    let mut bound_address = [0u8; 257];
    io.read_exact(&mut bound_address[..length + 2])
        .await
        .map_err(|_| Error::Network)?;
    Ok(())
}
