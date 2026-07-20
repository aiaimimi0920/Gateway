use serde::Serialize;
use std::process::Child;
use std::sync::{Arc, Mutex};

use crate::process::shutdown_process_runtime;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayProcessSnapshot {
    pub running: bool,
    pub pid: Option<u32>,
    pub port: Option<u16>,
    pub profile_name: Option<String>,
    pub log_path: Option<String>,
    pub started_at: Option<String>,
    pub startup_state: Option<String>,
    pub shutdown_state: Option<String>,
    pub last_error: Option<String>,
    pub recent_log_lines: Vec<String>,
}

#[derive(Debug, Default)]
pub struct GatewayProcessRuntime {
    pub child: Option<Child>,
    pub management_token: Option<String>,
    pub snapshot: GatewayProcessSnapshot,
}

#[derive(Clone, Default)]
pub struct GatewayDesktopState {
    pub process: Arc<Mutex<GatewayProcessRuntime>>,
}

impl Drop for GatewayDesktopState {
    fn drop(&mut self) {
        if Arc::strong_count(&self.process) > 1 {
            return;
        }
        let Ok(mut runtime) = self.process.lock() else {
            return;
        };
        shutdown_process_runtime(&mut runtime);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{ErrorKind, Read, Write};
    use std::net::TcpListener;
    use std::process::Command;
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    #[test]
    fn drop_requests_authorized_drain_before_waiting_for_child_exit() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind drain fixture");
        listener
            .set_nonblocking(true)
            .expect("set drain fixture nonblocking");
        let port = listener
            .local_addr()
            .expect("read drain fixture port")
            .port();
        let (request_tx, request_rx) = mpsc::channel();
        let server = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(3);
            while Instant::now() < deadline {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream
                            .set_read_timeout(Some(Duration::from_secs(1)))
                            .expect("set drain request read timeout");
                        let mut request = Vec::new();
                        let mut chunk = [0_u8; 1024];
                        while request.len() < 16 * 1024 {
                            match stream.read(&mut chunk) {
                                Ok(0) => break,
                                Ok(count) => {
                                    request.extend_from_slice(&chunk[..count]);
                                    if request.windows(4).any(|window| window == b"\r\n\r\n") {
                                        break;
                                    }
                                }
                                Err(error)
                                    if matches!(
                                        error.kind(),
                                        ErrorKind::WouldBlock | ErrorKind::TimedOut
                                    ) =>
                                {
                                    break;
                                }
                                Err(error) => panic!("read drain request: {error}"),
                            }
                        }
                        let request_text = String::from_utf8_lossy(&request).to_ascii_lowercase();
                        let authorized =
                            request_text.contains("x-management-token: management-secret");
                        stream
                            .write_all(
                                b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                            )
                            .expect("write drain response");
                        request_tx.send(authorized).expect("report drain request");
                        return;
                    }
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(20));
                    }
                    Err(error) => panic!("accept drain request: {error}"),
                }
            }
            request_tx
                .send(false)
                .expect("report missing drain request");
        });

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock must be after unix epoch")
            .as_nanos();
        let marker_path = std::env::temp_dir().join(format!("gateway-drop-graceful-{nonce}.txt"));
        let mut command = if cfg!(windows) {
            let escaped_path = marker_path.display().to_string().replace('\'', "''");
            let mut command = Command::new("powershell.exe");
            command.args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                &format!(
                    "Start-Sleep -Milliseconds 500; [IO.File]::WriteAllText('{escaped_path}', 'graceful')"
                ),
            ]);
            command
        } else {
            let mut command = Command::new("sh");
            command.args([
                "-c",
                &format!("sleep 0.5; printf graceful > '{}'", marker_path.display()),
            ]);
            command
        };
        let child = command.spawn().expect("spawn graceful child fixture");
        let child_id = child.id();
        let state = GatewayDesktopState {
            process: Arc::new(Mutex::new(GatewayProcessRuntime {
                child: Some(child),
                management_token: Some("management-secret".to_string()),
                snapshot: GatewayProcessSnapshot {
                    running: true,
                    pid: Some(child_id),
                    port: Some(port),
                    ..GatewayProcessSnapshot::default()
                },
            })),
        };

        drop(state);

        assert!(
            request_rx
                .recv_timeout(Duration::from_secs(4))
                .expect("receive drain request result"),
            "Drop must send an authorized drain request before process termination"
        );
        server.join().expect("join drain fixture");
        assert_eq!(
            std::fs::read_to_string(&marker_path).expect("graceful child marker"),
            "graceful"
        );
        let _ = std::fs::remove_file(marker_path);
    }
}
