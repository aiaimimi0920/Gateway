use crate::credential_pool_automation::{
    DriverCredentialContext, DriverProviderContext, DriverRefillContext, DriverRequest,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

const MAX_REQUEST_BYTES: usize = 16 * 1024;

pub(super) struct Reply {
    pub status: &'static str,
    pub declared_length: Option<usize>,
    pub body: Vec<u8>,
    pub hold_open: bool,
}

impl Reply {
    pub fn fixed(body: Vec<u8>) -> Self {
        Self {
            status: "200 OK",
            declared_length: Some(body.len()),
            body,
            hold_open: false,
        }
    }

    pub fn chunked(body: Vec<u8>, hold_open: bool) -> Self {
        Self {
            status: "200 OK",
            declared_length: None,
            body,
            hold_open,
        }
    }
}

pub(super) struct ObservedRequest {
    pub request_line: String,
    pub content_type: String,
    pub body: serde_json::Value,
}

pub(super) struct Server {
    pub endpoint: String,
    observed: Arc<Mutex<Option<ObservedRequest>>>,
    task: Option<JoinHandle<()>>,
}

impl Server {
    pub async fn start(reply: Reply) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/driver", listener.local_addr().unwrap());
        let observed = Arc::new(Mutex::new(None));
        let captured = Arc::clone(&observed);
        let task = tokio::spawn(async move {
            let _ = tokio::time::timeout(Duration::from_secs(12), async move {
                let (mut socket, _) = listener.accept().await?;
                let request = read_request(&mut socket).await?;
                *captured.lock().unwrap() = Some(request);
                write_reply(&mut socket, reply).await
            })
            .await;
        });
        Self {
            endpoint,
            observed,
            task: Some(task),
        }
    }

    pub async fn finish(mut self) -> ObservedRequest {
        if let Some(task) = self.task.take() {
            task.abort();
            let _ = task.await;
        }
        let observed = self.observed.lock().unwrap().take();
        observed.expect("loopback driver request was not captured")
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}

async fn read_request(socket: &mut TcpStream) -> anyhow::Result<ObservedRequest> {
    let mut bytes = Vec::new();
    let header_end = loop {
        if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
            break end + 4;
        }
        read_more(socket, &mut bytes).await?;
    };
    let headers = String::from_utf8(bytes[..header_end].to_vec())?;
    let field = |name: &str| {
        headers
            .lines()
            .filter_map(|line| line.split_once(':'))
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.trim().to_string())
    };
    let length = field("Content-Length")
        .ok_or_else(|| anyhow::anyhow!("missing request length"))?
        .parse::<usize>()?;
    let total = header_end
        .checked_add(length)
        .filter(|total| *total <= MAX_REQUEST_BYTES)
        .ok_or_else(|| anyhow::anyhow!("loopback request exceeded the fixture limit"))?;
    while bytes.len() < total {
        read_more(socket, &mut bytes).await?;
    }
    Ok(ObservedRequest {
        request_line: headers.lines().next().unwrap_or_default().to_string(),
        content_type: field("Content-Type").unwrap_or_default(),
        body: serde_json::from_slice(&bytes[header_end..total])?,
    })
}

async fn read_more(socket: &mut TcpStream, bytes: &mut Vec<u8>) -> anyhow::Result<()> {
    let remaining = MAX_REQUEST_BYTES.saturating_sub(bytes.len());
    anyhow::ensure!(remaining > 0, "loopback request exceeded the fixture limit");
    let mut buffer = [0u8; 1024];
    let capacity = buffer.len().min(remaining);
    let count = socket.read(&mut buffer[..capacity]).await?;
    anyhow::ensure!(count > 0, "loopback request ended early");
    bytes.extend_from_slice(&buffer[..count]);
    Ok(())
}

async fn write_reply(socket: &mut TcpStream, reply: Reply) -> anyhow::Result<()> {
    let framing = match reply.declared_length {
        Some(length) => format!("Content-Length: {length}\r\n"),
        None => "Transfer-Encoding: chunked\r\n".to_string(),
    };
    let header = format!(
        "HTTP/1.1 {}\r\nContent-Type: application/json\r\n{framing}Connection: close\r\n\r\n",
        reply.status
    );
    socket.write_all(header.as_bytes()).await?;
    for chunk in reply.body.chunks(32 * 1024) {
        if reply.declared_length.is_none() {
            socket
                .write_all(format!("{:x}\r\n", chunk.len()).as_bytes())
                .await?;
        }
        socket.write_all(chunk).await?;
        if reply.declared_length.is_none() {
            socket.write_all(b"\r\n").await?;
        }
    }
    if reply.hold_open {
        // The client must decide from the limit while this response remains open.
        std::future::pending::<()>().await;
    } else if reply.declared_length.is_none() {
        socket.write_all(b"0\r\n\r\n").await?;
    }
    socket.shutdown().await?;
    Ok(())
}

pub(super) fn request() -> DriverRequest {
    DriverRequest {
        run_id: "fixed-run".to_string(),
        action: "collect_refill",
        provider: DriverProviderContext {
            id: "provider-a".to_string(),
            label: "Provider A".to_string(),
            target_size: 3,
            credential_count: 1,
            active_credential_count: 1,
            requested_count: 2,
            auto_refill_enabled: true,
            auto_prune_enabled: false,
            identity_categories: vec![serde_json::json!({"id": "standard"})],
            credentials: vec![DriverCredentialContext {
                id: "existing-a".to_string(),
                account_name: Some("Existing A".to_string()),
                enabled: true,
                identity_category_id: Some("standard".to_string()),
            }],
        },
        refill: Some(DriverRefillContext {
            task_id: "task-a".to_string(),
            requested_count: 2,
            artifact_reference: "artifact-a".to_string(),
        }),
    }
}
