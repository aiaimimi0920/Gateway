//! A real TCP consumer exposes provisional bytes and bounded evidence, never approves purchases.
use super::*;
use futures::StreamExt;
use std::sync::{Arc, Mutex};
pub(super) struct Caller {
    pub task: tokio::task::JoinHandle<(u16, String)>,
    bytes: Arc<Mutex<Vec<u8>>>,
    terminal: &'static str,
}
pub(super) fn path(index: usize) -> &'static str {
    ["/v1/chat/completions", "/v1/responses", "/v1/messages"][index]
}
pub(super) fn input(index: usize) -> Value {
    if index == 1 {
        json!({"model":"open","input":"hello","stream":true})
    } else {
        json!({"model":"open","messages":[{"role":"user","content":"hello"}],"max_tokens":16,"stream":true})
    }
}
impl Caller {
    pub fn start(host: &Host, index: usize) -> Self {
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let output = bytes.clone();
        let client = host.client.clone();
        let token = host.token.clone();
        let url = format!("{}{}", host.server.origin, path(index));
        let task = tokio::spawn(async move {
            let response = client
                .post(url)
                .bearer_auth(token)
                .header("cookie", "forged-caller-cookie")
                .json(&input(index))
                .send()
                .await
                .unwrap();
            let status = response.status().as_u16();
            if status != 200 {
                return (status, response.text().await.unwrap());
            }
            assert!(response.headers()["content-type"]
                .to_str()
                .unwrap()
                .starts_with("text/event-stream"));
            let mut stream = response.bytes_stream();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.unwrap();
                let mut output = output.lock().unwrap();
                assert!(
                    output.len() + chunk.len() <= 128 * 1024,
                    "Bounded fixture output"
                );
                output.extend_from_slice(&chunk);
            }
            let text = String::from_utf8(output.lock().unwrap().clone()).unwrap();
            (status, text)
        });
        Self {
            task,
            bytes,
            terminal: [
                "data: [DONE]",
                "event: response.completed",
                "event: message_stop",
            ][index],
        }
    }
    pub fn snapshot(&self) -> String {
        String::from_utf8(self.bytes.lock().unwrap().clone()).unwrap()
    }
    pub async fn live(&self) -> String {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let text = self.snapshot();
                assert!(
                    !self.task.is_finished(),
                    "Public caller cannot end before native EOF/FINISH"
                );
                assert!(!text.contains(self.terminal));
                if text.contains("partial") && text.contains("native thought") {
                    break text;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap()
    }
    pub async fn completed(self) -> String {
        let (status, text) = tokio::time::timeout(Duration::from_secs(10), self.task)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(status, 200, "{text}");
        assert_eq!(text.matches(self.terminal).count(), 1);
        assert!(text.contains("partial") && text.contains("native thought"));
        text
    }
    pub async fn cancelled(self) {
        let (status, text) = tokio::time::timeout(Duration::from_secs(10), self.task)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(status, 409, "{text}");
        let error: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(error["error"]["code"], "freebuff_purchase_cancelled");
    }
}
