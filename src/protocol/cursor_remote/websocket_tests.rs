//! Real loopback WS/router/SQLite with a synthetic pure-Rust worker; no live upstream.
use super::*;
use futures::{SinkExt, StreamExt};
use tokio_tungstenite::{
    tungstenite::{client::IntoClientRequest, Message},
    MaybeTlsStream, WebSocketStream,
};
type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;
#[path = "websocket_image_tests.rs"]
mod images;
#[path = "websocket_incremental_tests.rs"]
mod incremental;

struct Server {
    url: String,
    task: tokio::task::JoinHandle<()>,
}
impl Server {
    async fn start(state: Arc<AppState>) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}/v1/responses", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, crate::http::router::build_router(state))
                .await
                .unwrap();
        });
        Self { url, task }
    }
    async fn connect(&self, token: &str) -> Socket {
        let mut request = self.url.as_str().into_client_request().unwrap();
        request
            .headers_mut()
            .insert("authorization", format!("Bearer {token}").parse().unwrap());
        tokio::time::timeout(
            Duration::from_secs(5),
            tokio_tungstenite::connect_async(request),
        )
        .await
        .unwrap()
        .unwrap()
        .0
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn turn(socket: &mut Socket, body: Value) -> Value {
    tokio::time::timeout(Duration::from_secs(5), async {
        socket.send(Message::Text(body.to_string())).await.unwrap();
        for _ in 0..64 {
            let frame = socket.next().await.unwrap().unwrap();
            if !frame.is_text() {
                continue;
            }
            let event: Value = serde_json::from_str(frame.to_text().unwrap()).unwrap();
            if matches!(
                event["type"].as_str(),
                Some("response.completed" | "error" | "response.failed")
            ) {
                return event;
            }
        }
        panic!("missing terminal event");
    })
    .await
    .unwrap()
}
fn body(input: Value, custom: bool) -> Value {
    let tool = if custom {
        json!({"type":"custom","name":"edit"})
    } else {
        json!({"type":"function","name":"echo","parameters":{"type":"object"}})
    };
    json!({"type":"response.create","model":"cursor-personal","store":false,
        "input":input,"tools":[tool]})
}

fn ws_tool_worker() -> Runtime {
    Runtime::fixture(|input, hooks| {
        Box::pin(async move {
            hooks.authorize().await?;
            hooks.sent()?;
            let name = input["tools"][0]["name"].as_str().unwrap().to_owned();
            let args = if name == "edit" {
                json!({"input":" \r\n补丁\n "}).to_string()
            } else {
                "{}".into()
            };
            let output = hooks
                .tool(WorkerToolCall {
                    call_id: "call_ws".into(),
                    name,
                    arguments: args,
                    text: String::new(),
                    thinking: None,
                    requested_model: "default".into(),
                })
                .await?;
            if output.output != "ok" {
                return Err(super::super::error("worker_failed"));
            }
            hooks.continued()?;
            Ok(WorkerOutput {
                usage: None,
                text: "done".into(),
                thinking: None,
                requested_model: "default".into(),
            })
        })
    })
}

#[test]
fn cursor_remote_websocket_text_turns_reject_storage_and_preserve_socket() {
    run(|| async {
        let state = app(true).await;
        let token = key(&state, "responses", "cursor-remote").await;
        let (status, rejected) = post_to(
            &state,
            &token,
            "/v1/responses/compact",
            json!({"model":"cursor-personal","store":false,"input":"hello"}),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{rejected}");
        assert!(rejected.contains("cursor_remote_compact_unsupported"));
        let server = Server::start(state.clone()).await;
        let mut socket = server.connect(&token).await;
        for key in ["store", "previous_response_id"] {
            let mut request = json!({"type":"response.create","model":"cursor-personal",
                "store":false,"input":"hello"});
            request[key] = if key == "store" {
                json!(true)
            } else {
                json!("resp_missing")
            };
            assert_eq!(turn(&mut socket, request).await["type"], "error");
        }
        for _ in 0..2 {
            let event = turn(
                &mut socket,
                json!({"type":"response.create","model":"cursor-personal",
                "store":false,"input":"hello"}),
            )
            .await;
            assert_eq!(event["type"], "response.completed", "{event}");
            assert!(event.to_string().contains("remote fixture"));
        }
        socket.close(None).await.unwrap();
        drop(socket);
        state.lifecycle.cursor_remote.join().await;
        drop(server);
        state.local_runtime.as_ref().unwrap().close().await;
    });
}

#[test]
fn cursor_remote_websocket_function_custom_cycles_are_bound_to_the_socket() {
    run(|| async {
        let state = app_worker(true, ws_tool_worker()).await;
        let token = key(&state, "responses", "cursor-remote").await;
        let server = Server::start(state.clone()).await;
        let mut socket = server.connect(&token).await;
        let mut other = server.connect(&token).await;
        for custom in [false, true] {
            let first = turn(&mut socket, body(json!("hello"), custom)).await;
            assert_eq!(first["type"], "response.completed", "{first}");
            let call = &first["response"]["output"][0];
            assert_eq!(call["call_id"], "call_ws");
            assert_eq!(
                call["type"],
                if custom {
                    "custom_tool_call"
                } else {
                    "function_call"
                }
            );
            if custom {
                assert_eq!(call["input"], " \r\n补丁\n ");
            }
            let mut history = vec![json!({"role":"user","content":"hello"})];
            history.extend(first["response"]["output"].as_array().unwrap().clone());
            history.push(
                json!({"type":if custom {"custom_tool_call_output"} else {"function_call_output"},
                "call_id":"call_ws","output":"ok"}),
            );
            let continuation = body(json!(history), custom);
            assert_eq!(
                turn(&mut other, continuation.clone()).await["type"],
                "error"
            );
            let mut http = continuation.clone();
            http.as_object_mut().unwrap().remove("type");
            assert_ne!(post(&state, &token, http).await.0, StatusCode::OK);
            let done = turn(&mut socket, continuation.clone()).await;
            assert_eq!(done["type"], "response.completed", "{done}");
            assert!(done.to_string().contains("done"));
            assert_eq!(turn(&mut socket, continuation).await["type"], "error");
        }
        socket.close(None).await.unwrap();
        other.close(None).await.unwrap();
        drop((socket, other));
        state.lifecycle.cursor_remote.join().await;
        drop(server);
        state.local_runtime.as_ref().unwrap().close().await;
    });
}

#[test]
fn cursor_remote_websocket_disconnect_cancels_published_tool_and_frees_slot() {
    run(|| async {
        let state = app_worker(true, ws_tool_worker()).await;
        let token = key(&state, "responses", "cursor-remote").await;
        let server = Server::start(state.clone()).await;
        for _ in 0..2 {
            let mut socket = server.connect(&token).await;
            let response = turn(&mut socket, body(json!("hello"), false)).await;
            assert_eq!(response["type"], "response.completed", "{response}");
            socket.close(None).await.unwrap();
            drop(socket);
            tokio::time::timeout(
                Duration::from_secs(5),
                state.lifecycle.cursor_remote.fixture_wait_idle(),
            )
            .await
            .unwrap();
            assert!(state.lifecycle.cursor_remote.available());
        }
        state.lifecycle.cursor_remote.join().await;
        drop(server);
        state.local_runtime.as_ref().unwrap().close().await;
    });
}
