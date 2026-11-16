//! All public protocols share the original paid source; no terminal precedes EOF and FINISH ack.
use super::*;
use futures::StreamExt;
mod controls;
mod failures;
mod semantics;
const PATHS: [&str; 2] = ["/v1/responses", "/v1/messages"];
fn input(path: &str) -> Value {
    if path == PATHS[0] {
        json!({"model":"open","input":"hello","stream":true})
    } else {
        json!({"model":"open","messages":[{"role":"user","content":"hello"}],"max_tokens":16,"stream":true})
    }
}
fn request(host: &Host, path: &str, body: Value) -> tokio::task::JoinHandle<rquest::Response> {
    let client = host.client.clone();
    let token = host.token.clone();
    let url = format!("{}{path}", host.server.origin);
    tokio::spawn(async move {
        client
            .post(url)
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .unwrap()
    })
}
fn terminal(path: &str) -> &'static str {
    if path == PATHS[0] {
        "event: response.completed"
    } else {
        "event: message_stop"
    }
}
#[tokio::test]
async fn freebuff_public_protocol_sse_deltas_precede_eof_and_terminal_waits_for_original_finish() {
    for path in PATHS {
        let peer = peer::Peer::new().await;
        *peer.mode.lock().unwrap() = "public_sse_known".into();
        let host = Box::pin(Host::new(&peer)).await;
        host.seed_device(&peer).await;
        let pause = peer.pause("/api/v1/agent-runs", 2);
        let task = request(&host, path, input(path));
        public_sse::approve(&host, &task).await;
        let response = task.await.unwrap();
        assert_eq!(response.status(), 200);
        let mut stream = response.bytes_stream();
        let mut before = String::new();
        tokio::time::timeout(Duration::from_secs(3), async {
            while !before.contains("native thought") {
                before
                    .push_str(std::str::from_utf8(&stream.next().await.unwrap().unwrap()).unwrap());
            }
        })
        .await
        .unwrap();
        assert!(before.contains("partial"));
        assert!(before.contains("reasoning_details"));
        assert!(before.contains("gateway_freebuff.delta"));
        assert!(!before.contains(terminal(path)));
        assert!(!before.contains("signature_delta"));
        assert!(!before.contains("thinking_delta"));
        assert!(peer.finishes().is_empty());
        peer.live.eof.notify_one();
        let read = async {
            let mut rest = Vec::new();
            while let Some(bytes) = stream.next().await {
                rest.extend_from_slice(&bytes.unwrap());
            }
            String::from_utf8(rest).unwrap()
        };
        tokio::pin!(read);
        tokio::select! { _=&mut read=>panic!("terminal preceded FINISH ack"), _=pause.reached()=>{} }
        assert_eq!(peer.finishes()[0]["totalCredits"], 13);
        pause.resume();
        let rest = tokio::time::timeout(Duration::from_secs(3), read)
            .await
            .unwrap();
        assert!(rest.contains(terminal(path)), "{path}: {rest}");
        assert_eq!(peer.calls("/api/v1/chat/completions"), 1);
        let wire = peer
            .hits
            .lock()
            .unwrap()
            .iter()
            .find(|(_, p, _, _)| p == "/api/v1/chat/completions")
            .unwrap()
            .3
            .clone();
        assert_eq!(wire["stream"], true);
        settled(&host, "completed", 9).await;
        host.close().await;
        peer.server.close().await;
    }
}
