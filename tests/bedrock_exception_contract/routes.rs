//! Genuine public TCP, ordinary Local Key, failed audit/refund and a native terminal error frame.
use super::{
    failure,
    runtime::{Fixture, DEADLINE},
    wire, CASES, PRIVATE,
};
use crc::{Crc, CRC_32_ISO_HDLC};
use futures::StreamExt;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use tokio::time::timeout;

fn run<F: std::future::Future<Output = ()>>(build: impl FnOnce() -> F + Send + 'static) {
    // Only this integration harness uses the existing explicit stack budget.
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    timeout(std::time::Duration::from_secs(90), build())
                        .await
                        .unwrap()
                });
        })
        .unwrap()
        .join()
        .unwrap();
}

fn decode(mut bytes: &[u8]) -> Vec<(BTreeMap<String, String>, Value)> {
    let mut frames = Vec::new();
    let crc = Crc::<u32>::new(&CRC_32_ISO_HDLC);
    while !bytes.is_empty() {
        assert!(bytes.len() >= 16);
        let number =
            |offset| u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        let length = number(0);
        let end = 12 + number(4);
        assert!(length <= bytes.len() && end <= length - 4);
        assert_eq!(number(8) as u32, crc.checksum(&bytes[..8]));
        assert_eq!(
            number(length - 4) as u32,
            crc.checksum(&bytes[..length - 4])
        );
        let mut cursor = 12;
        let mut headers = BTreeMap::new();
        while cursor < end {
            let size = bytes[cursor] as usize;
            cursor += 1;
            let name = std::str::from_utf8(&bytes[cursor..cursor + size])
                .unwrap()
                .to_owned();
            cursor += size;
            assert_eq!(bytes[cursor], 7);
            cursor += 1;
            let size = u16::from_be_bytes(bytes[cursor..cursor + 2].try_into().unwrap()) as usize;
            cursor += 2;
            let value = std::str::from_utf8(&bytes[cursor..cursor + size])
                .unwrap()
                .to_owned();
            cursor += size;
            assert!(headers.insert(name, value).is_none());
        }
        assert_eq!(cursor, end);
        frames.push((
            headers,
            serde_json::from_slice(&bytes[end..length - 4]).unwrap(),
        ));
        bytes = &bytes[length..];
    }
    frames
}

#[test]
fn native_tcp_modeled_exceptions_are_valid_terminal_frames_but_settle_failed_once() {
    run(|| async {
        let fixture = Fixture::start().await;
        for (index, &(kind, _, _, _)) in CASES.iter().enumerate() {
            let mut bytes = wire::sequence()[..2].concat();
            bytes.extend(failure(kind));
            fixture.reply(bytes);
            let response = fixture
                .open(
                    "/model/fixture-model/converse-stream",
                    json!({"messages":[{"role":"user","content":[{"text":"hello"}]}]}),
                )
                .await;
            assert_eq!(response.status().as_u16(), 200);
            assert_eq!(
                response.headers()["content-type"],
                "application/vnd.amazon.eventstream"
            );
            let body = timeout(DEADLINE, response.bytes())
                .await
                .unwrap()
                .expect("native exception must not truncate the HTTP body");
            assert!(!String::from_utf8_lossy(&body).contains(PRIVATE));
            let frames = decode(&body);
            assert_eq!(frames.len(), 3);
            assert_eq!(frames[0].0[":event-type"], "messageStart");
            let (headers, payload) = frames.last().unwrap();
            assert_eq!(headers.len(), 3);
            assert_eq!(headers[":message-type"], "exception");
            assert_eq!(headers[":exception-type"], kind);
            assert!(!headers.contains_key(":event-type"));
            assert_eq!(headers[":content-type"], "application/json");
            assert!(payload["message"].is_string());
            assert!(payload.get("originalMessage").is_none());
            assert!(frames.iter().all(|(h, _)| !matches!(
                h.get(":event-type").map(String::as_str),
                Some("messageStop" | "metadata")
            )));
            fixture.settled(index + 1).await;
        }
        fixture.close().await;
    });
}

#[test]
fn public_tcp_buffered_exception_keeps_semantic_status_and_never_replays_or_charges() {
    run(|| async {
        let fixture = Fixture::start().await;
        for (index, &(kind, code, status, _)) in CASES.iter().enumerate() {
            fixture.reply(failure(kind));
            let response = fixture
                .open(
                    "/v1/chat/completions",
                    json!({"model":"fixture-model","messages":[{"role":"user","content":"hello"}]}),
                )
                .await;
            assert_eq!(response.status().as_u16(), status, "{kind}");
            let body: Value = response.json().await.unwrap();
            assert_eq!(body["error"]["code"], code);
            assert!(!body.to_string().contains(PRIVATE));
            fixture.settled(index + 1).await;
        }
        fixture.close().await;
    });
}

#[test]
fn three_public_tcp_sse_errors_preserve_failure_without_success_terminal_or_replay() {
    run(|| async {
        for (index, path) in ["/v1/chat/completions", "/v1/messages", "/v1/responses"]
            .iter()
            .enumerate()
        {
            // Separate accounts avoid testing the previous protocol's real rate-limit cooldown.
            let fixture = Fixture::start().await;
            fixture.held(
                wire::sequence()[..2].concat(),
                failure("throttlingException"),
            );
            let body = if index == 2 {
                json!({"model":"fixture-model","stream":true,"input":"hello"})
            } else {
                json!({"model":"fixture-model","stream":true,"max_tokens":8,"messages":[{"role":"user","content":"hello"}]})
            };
            let response = fixture.open(path, body).await;
            assert_eq!(response.status().as_u16(), 200);
            let mut stream = response.bytes_stream();
            let mut output = Vec::new();
            let mut failed = false;
            let first = timeout(DEADLINE, stream.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            output.extend(first);
            fixture.release();
            timeout(DEADLINE, async {
                while let Some(item) = stream.next().await {
                    match item {
                        Ok(bytes) => output.extend(bytes),
                        Err(_) => {
                            failed = true;
                            break;
                        }
                    }
                    assert!(output.len() < 64 * 1024);
                }
            })
            .await
            .unwrap();
            drop(stream);
            assert!(
                failed,
                "public SSE must remain failed rather than clean EOF"
            );
            let text = String::from_utf8(output).unwrap();
            assert!(!text.contains(PRIVATE));
            for success in [
                "[DONE]",
                "message_stop",
                "response.completed",
                "gateway_bedrock_terminal",
            ] {
                assert!(!text.contains(success), "{path}: {success}");
            }
            fixture.settled(1).await;
            fixture.close().await;
        }
    });
}
