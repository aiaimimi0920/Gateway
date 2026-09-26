use super::*;
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::time::timeout;

#[test]
fn clip_ids_preserve_first_seen_order_and_trimmed_alias_deduplication() {
    let expected: Vec<String> = (0..4096).map(|index| format!("clip-{index}")).collect();
    let entries: Vec<Value> = expected
        .iter()
        .flat_map(|id| [json!({"clip_id": id}), json!({"songId": format!(" {id} ")})])
        .collect();
    assert_eq!(producer_music_clip_ids(&Value::Array(entries)), expected);
    assert!(producer_music_clip_ids(&json!({"clipId": " ", "song_id": 3})).is_empty());
}

#[test]
fn completion_membership_preserves_duplicate_and_missing_id_contract() {
    let clips = vec![
        json!({"clip_id": "a", "audio_url": ""}),
        json!({"clip_id": "a", "wav_url": "https://example.test/a.wav"}),
        json!({"clip_id": "b", "audio_url": "https://example.test/b.mp3"}),
    ];
    assert!(producer_music_clip_assets_complete(
        &clips,
        &["b".into(), "a".into(), "a".into()]
    ));
    assert!(!producer_music_clip_assets_complete(
        &clips,
        &["missing".into()]
    ));
    assert!(!producer_music_clip_assets_complete(
        &clips,
        &[" a ".into()]
    ));
    assert!(producer_music_clip_assets_complete(&clips, &[]));
}

#[test]
fn unusable_audio_falls_back_to_downloadable_wav() {
    for audio in [json!(""), json!(" "), Value::Null, json!(7)] {
        let clips = vec![json!({
            "clip_id": "a", "audio_url": audio, "wav_url": "https://example.test/a.wav"
        })];
        assert!(producer_music_clip_assets_complete(&clips, &["a".into()]));
        let result = build_producer_music_completed_response(json!({}), clips).unwrap();
        assert_eq!(result["data"][0]["url"], "https://example.test/a.wav");
    }
}

struct Server(tokio::task::JoinHandle<()>);

impl Drop for Server {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn assert_poll_budget(stall_body: bool) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let (sent, arrived) = oneshot::channel();
    let _server = Server(tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0u8; 8192];
        assert!(socket.read(&mut request).await.unwrap() > 0);
        if stall_body {
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\n{")
                .await
                .unwrap();
        } else {
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}")
                .await
                .unwrap();
        }
        sent.send(()).unwrap();
        if stall_body {
            std::future::pending::<()>().await;
        }
    }));
    let client = Client::builder().build().unwrap();
    let result = timeout(
        Duration::from_secs(2),
        poll_producer_music_clip_assets(
            &client,
            &base_url,
            &HeaderMap::new(),
            json!({"clip_id": "a"}),
            Duration::from_millis(200),
        ),
    )
    .await
    .expect("the overall poll budget must include request bodies and interval sleep")
    .unwrap_err();
    arrived.await.unwrap();
    assert_eq!(
        result.code.as_deref(),
        Some("producer_music_clip_poll_timeout")
    );
}

#[tokio::test]
async fn poll_budget_includes_interval_sleep() {
    assert_poll_budget(false).await;
}

#[tokio::test]
async fn poll_budget_includes_stalled_response_body() {
    assert_poll_budget(true).await;
}
