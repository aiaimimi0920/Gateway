//! A bounded RESP witness for the exact production GET/CAS wire, not a general Redis implementation.
use super::*;
use std::sync::Mutex;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};

#[derive(Default)]
struct State {
    raw: String,
    writable: bool,
    loaded: bool,
    hash: Option<String>,
    commits: usize,
    failures: usize,
    event: Arc<tokio::sync::Notify>,
}
pub(super) struct Cache {
    pub pool: deadpool_redis::Pool,
    state: Arc<Mutex<State>>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<()>>,
}
impl Cache {
    pub async fn new(writable: bool) -> Self {
        let raw = serde_json::json!({"id":"exact-build","kind":"account_credential",
            "project_id":"synthetic-project","user_id":"synthetic-user","provider":"grok-oauth",
            "api_key":"synthetic-build-token","api_base_url":"https://cli-chat-proxy.grok.com/v1",
            "created_at":"2020-01-01T00:00:00Z","updated_at":"2020-01-01T00:00:00Z",
            "expires_at":"2020-01-01T00:00:00Z","quota_remaining_tokens":42,
            "account_payload":{"preset":"grok-oauth","refresh_token":"synthetic-refresh",
                "grok_oauth":grok_context::context()}})
        .to_string();
        let state = Arc::new(Mutex::new(State {
            raw,
            writable,
            ..Default::default()
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let pool =
            deadpool_redis::Config::from_url(format!("redis://{}", listener.local_addr().unwrap()))
                .create_pool(Some(deadpool_redis::Runtime::Tokio1))
                .unwrap();
        let (stop, mut stopped) = tokio::sync::oneshot::channel();
        let shared = state.clone();
        let task = tokio::spawn(async move {
            let mut workers = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    _ = &mut stopped => break,
                    accepted = listener.accept() => {
                        let (socket, _) = accepted.unwrap();
                        let state = shared.clone();
                        workers.spawn(connection(socket, state));
                    }
                    Some(result) = workers.join_next(), if !workers.is_empty() => {
                        // Cancelling a pooled request or closing its pool can reset this test socket.
                        if let Err(error) = result.unwrap() {
                            assert_eq!(error.kind(), std::io::ErrorKind::ConnectionReset,
                                "unexpected cache socket failure: {error}");
                        }
                    }
                }
            }
            workers.shutdown().await;
        });
        Self {
            pool,
            state,
            stop: Some(stop),
            task: Some(task),
        }
    }
    pub fn allow_writes(&self) {
        self.state.lock().unwrap().writable = true;
    }
    pub fn value(&self) -> serde_json::Value {
        serde_json::from_str(&self.state.lock().unwrap().raw).unwrap()
    }
    pub fn replace_raw(&self, value: serde_json::Value) {
        self.state.lock().unwrap().raw = value.to_string();
    }
    pub fn commits(&self) -> usize {
        self.state.lock().unwrap().commits
    }
    pub async fn wait_write_result(&self) {
        let event = self.state.lock().unwrap().event.clone();
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let notified = event.notified();
                tokio::pin!(notified);
                notified.as_mut().enable();
                {
                    let state = self.state.lock().unwrap();
                    if state.commits > 0 || state.failures > 0 {
                        return;
                    }
                }
                notified.await;
            }
        })
        .await
        .expect("cache did not receive the first write result");
    }
    pub async fn stop(mut self) {
        self.pool.close();
        let _ = self.stop.take().unwrap().send(());
        self.task.take().unwrap().await.unwrap();
    }
}
impl Drop for Cache {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}

async fn command(
    reader: &mut BufReader<tokio::net::TcpStream>,
) -> std::io::Result<Option<Vec<String>>> {
    let Some(header) = line(reader).await? else {
        return Ok(None);
    };
    assert!(header.starts_with('*'));
    let count = header[1..].trim().parse::<usize>().unwrap();
    assert!(count <= 16);
    let mut args = Vec::with_capacity(count);
    let mut total = 0;
    for _ in 0..count {
        let header = line(reader).await?.expect("complete fixture command");
        assert!(header.starts_with('$'));
        let length = header[1..].trim().parse::<usize>().unwrap();
        total += length;
        assert!(length <= 65_536 && total <= 131_072);
        let mut value = vec![0; length + 2];
        reader.read_exact(&mut value).await?;
        assert_eq!(&value[length..], b"\r\n");
        value.truncate(length);
        args.push(String::from_utf8(value).unwrap());
    }
    Ok(Some(args))
}
async fn line(reader: &mut BufReader<tokio::net::TcpStream>) -> std::io::Result<Option<String>> {
    let mut bytes = Vec::with_capacity(32);
    for _ in 0..32 {
        let mut byte = [0];
        if reader.read(&mut byte).await? == 0 {
            assert!(bytes.is_empty(), "truncated fixture header");
            return Ok(None);
        }
        bytes.push(byte[0]);
        if byte[0] == b'\n' {
            return Ok(Some(String::from_utf8(bytes).unwrap()));
        }
    }
    panic!("fixture RESP header exceeds bound");
}
async fn connection(
    socket: tokio::net::TcpStream,
    state: Arc<Mutex<State>>,
) -> std::io::Result<()> {
    let mut reader = BufReader::new(socket);
    while let Some(args) = command(&mut reader).await? {
        let response = {
            let mut state = state.lock().unwrap();
            let response = respond(&args, &mut state);
            state.event.notify_waiters();
            response
        };
        reader.get_mut().write_all(response.as_bytes()).await?;
    }
    Ok(())
}
fn bulk(value: &str) -> String {
    format!("${}\r\n{value}\r\n", value.len())
}
fn respond(args: &[String], state: &mut State) -> String {
    match args[0].as_str() {
        "CLIENT" => "+OK\r\n".into(),
        // deadpool-redis recycles a connection with UNWATCH followed by PING <nonce>.
        "UNWATCH" => {
            assert_eq!(args.len(), 1);
            "+OK\r\n".into()
        }
        "PING" => {
            assert!((1..=2).contains(&args.len()));
            args.get(1)
                .map_or_else(|| "+PONG\r\n".into(), |value| bulk(value))
        }
        "GET" => {
            assert_eq!(args[1], crate::redis::keys::credential_key("exact-build"));
            bulk(&state.raw)
        }
        "SCRIPT" => {
            assert_eq!(args[1], "LOAD");
            assert_eq!(args[2], super::super::server_cache::CAS);
            state.loaded = true;
            bulk(
                state
                    .hash
                    .as_ref()
                    .expect("first EVALSHA records the exact script hash"),
            )
        }
        "EVALSHA" => {
            if !state.writable {
                state.failures += 1;
                return "-ERR fixture-cache-unavailable\r\n".into();
            }
            if !state.loaded {
                state.hash = Some(args[1].clone());
                return "-NOSCRIPT No matching script\r\n".into();
            }
            assert_eq!(Some(&args[1]), state.hash.as_ref());
            assert_eq!(args.len(), 7);
            assert_eq!(args[2], "1");
            assert_eq!(args[3], crate::redis::keys::credential_key("exact-build"));
            assert!(args[6].parse::<u64>().unwrap() > 0);
            if state.raw != args[4] {
                return ":0\r\n".into();
            }
            state.raw = args[5].clone();
            state.commits += 1;
            ":1\r\n".into()
        }
        _ => panic!("unsupported fixture Redis command: {}", args[0]),
    }
}

#[test]
fn pool_recycle_replies_preserve_ping_nonce_without_changing_cache() {
    let mut state = State {
        raw: "unchanged".into(),
        ..Default::default()
    };
    assert_eq!(respond(&["UNWATCH".into()], &mut state), "+OK\r\n");
    assert_eq!(
        respond(&["PING".into(), "7".into()], &mut state),
        "$1\r\n7\r\n"
    );
    assert_eq!(respond(&["PING".into()], &mut state), "+PONG\r\n");
    assert_eq!(state.raw, "unchanged");
    assert_eq!(state.commits, 0);
}
