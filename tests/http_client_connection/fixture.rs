//! 有界 loopback HTTP/1 夹具；只处理本组测试的固定请求，不启动后台 server task。
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

pub const DEADLINE: std::time::Duration = std::time::Duration::from_secs(5);

pub struct Request {
    pub method: String,
    pub target: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

pub async fn receive(socket: &mut TcpStream) -> Request {
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        assert!(
            bytes.len() < 8192,
            "fixture request headers exceeded budget"
        );
        bytes.push(socket.read_u8().await.unwrap());
    }
    let text = String::from_utf8(bytes).unwrap();
    let mut lines = text.split("\r\n");
    let mut start = lines.next().unwrap().split_whitespace();
    let method = start.next().unwrap().to_owned();
    let target = start.next().unwrap().to_owned();
    let headers: Vec<_> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.to_owned(), value.trim().to_owned()))
        .collect();
    let count = headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("content-length"))
        .map(|(_, value)| value.parse::<usize>().unwrap())
        .unwrap_or(0);
    assert!(count <= 16 * 1024, "fixture request body exceeded budget");
    let mut body = vec![0; count];
    socket.read_exact(&mut body).await.unwrap();
    Request {
        method,
        target,
        headers,
        body,
    }
}

pub async fn reply(socket: &mut TcpStream, status: &str, headers: &str, body: &str, close: bool) {
    let connection = if close { "close" } else { "keep-alive" };
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: {connection}\r\n{headers}\r\n",
        body.len()
    );
    socket.write_all(head.as_bytes()).await.unwrap();
    socket.write_all(body.as_bytes()).await.unwrap();
}
