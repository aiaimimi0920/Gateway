use super::{test_support::*, *};
use axum::{
    body::{Body, Bytes},
    response::Response,
};
use serde_json::json;

#[tokio::test]
async fn qwen_catalog_rejects_oversized_declared_length_before_reading_and_body_errors() {
    let pending = futures::stream::pending::<Result<Bytes, std::io::Error>>();
    let response = Response::builder()
        .header("content-length", (LIMIT + 1).to_string())
        .body(Body::from_stream(pending))
        .unwrap();
    let server = Server::start(vec![response]).await;
    assert!(tokio::time::timeout(Duration::from_secs(3), server.fetch())
        .await
        .unwrap()
        .is_err());
    let prefix = Bytes::from(page(1, 1, vec![row(0)]).to_string());
    let stream = futures::stream::iter(vec![
        Ok(prefix),
        Err(std::io::Error::other("synthetic-body-secret")),
    ]);
    let server = Server::start(vec![Response::new(Body::from_stream(stream))]).await;
    let error = server.fetch().await.unwrap_err();
    assert!(!error.to_string().contains("synthetic-body-secret"));
    assert_eq!(server.calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn qwen_whole_round_deadline_is_not_reset_by_the_next_page() {
    let bytes = Bytes::from(page(1, 21, (0..20).map(row).collect()).to_string());
    let first = futures::stream::once(async move {
        tokio::time::sleep(Duration::from_secs(12)).await;
        Ok::<_, std::io::Error>(bytes)
    });
    let second = futures::stream::pending::<Result<Bytes, std::io::Error>>();
    let server = Server::start(vec![
        Response::new(Body::from_stream(first)),
        Response::new(Body::from_stream(second)),
    ])
    .await;
    // Page two's 25-second timeout would expire at ~37s; the whole round must stop first.
    let result = tokio::time::timeout(Duration::from_secs(34), server.fetch())
        .await
        .unwrap();
    assert!(result.is_err());
    assert_eq!(server.calls.lock().unwrap().len(), 2);
}

struct StalledBody {
    first: Option<Bytes>,
    started: Option<tokio::sync::oneshot::Sender<()>>,
    released: Option<tokio::sync::oneshot::Sender<()>>,
}
impl futures::Stream for StalledBody {
    type Item = Result<Bytes, std::io::Error>;
    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        if let Some(bytes) = self.first.take() {
            return std::task::Poll::Ready(Some(Ok(bytes)));
        }
        if let Some(started) = self.started.take() {
            let _ = started.send(());
        }
        std::task::Poll::Pending
    }
}
impl Drop for StalledBody {
    fn drop(&mut self) {
        if let Some(released) = self.released.take() {
            let _ = released.send(());
        }
    }
}

#[tokio::test]
async fn cancelling_qwen_catalog_releases_the_active_body_without_requesting_another_page() {
    let (started, ready) = tokio::sync::oneshot::channel();
    let (released, closed) = tokio::sync::oneshot::channel();
    let stream = StalledBody {
        first: Some(Bytes::from_static(b"{\"success\":true,")),
        started: Some(started),
        released: Some(released),
    };
    let server = Server::start(vec![Response::new(Body::from_stream(stream))]).await;
    let source = server.url.clone();
    let task = tokio::spawn(async move {
        let client = rquest::Client::builder()
            .no_proxy()
            .redirect(rquest::redirect::Policy::none())
            .build()
            .unwrap();
        fetch_from(
            &client,
            &source,
            "synthetic-qwen-key",
            &QwenEndpoint::parse(BASE).unwrap(),
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(5), ready)
        .await
        .unwrap()
        .unwrap();
    assert!(!task.is_finished());
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    tokio::time::timeout(Duration::from_secs(5), closed)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(server.calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn qwen_native_pagination_rejects_inconsistent_or_incomplete_success() {
    let valid = page(1, 1, vec![row(0)]);
    let mut cases = Vec::new();
    for (field, value) in [
        ("success", json!(false)),
        ("code", json!("SyntheticSecretFailure")),
    ] {
        let mut body = valid.clone();
        body[field] = value;
        cases.push(vec![reply(body)]);
    }
    for (field, value) in [
        ("total", json!(2049)),
        ("total", json!(0)),
        ("total", json!(-1)),
        ("page_no", json!(2)),
        ("page_size", json!(100)),
        ("models", json!([])),
    ] {
        let mut body = valid.clone();
        body["output"][field] = value;
        cases.push(vec![reply(body)]);
    }
    cases.push(vec![
        reply(page(1, 21, (0..20).map(row).collect())),
        reply(page(2, 22, vec![row(20), row(21)])),
    ]);
    cases.push(vec![
        reply(page(1, 21, (0..20).map(row).collect())),
        reply(page(1, 21, vec![row(20)])),
    ]);
    cases.push(vec![
        reply(page(1, 21, (0..20).map(row).collect())),
        reply(page(2, 21, vec![])),
    ]);
    cases.push(vec![
        reply(page(1, 21, (0..20).map(row).collect())),
        reply(page(2, 21, vec![row(0)])),
    ]);
    cases.push(vec![reply(page(1, 2, vec![row(0), row(0)]))]);
    for replies in cases {
        let server = Server::start(replies).await;
        let error = server.fetch().await.unwrap_err();
        assert!(!error.to_string().contains("SyntheticSecretFailure"));
        assert!(server.calls.lock().unwrap().len() <= 2);
    }
}

#[tokio::test]
async fn qwen_metadata_never_turns_empty_unsafe_or_nonchat_rows_into_routes() {
    for malformed in [
        json!({"model":"*"}),
        json!({"model":"a".repeat(257)}),
        json!({"model":"bad\nmodel"}),
        json!({"model":"fixture","provider":123}),
        json!({"model":"fixture","capabilities":[1]}),
        json!({"model":"fixture","inference_metadata":{"request_modality":[1],"response_modality":["Text"]}}),
        json!({"model":"fixture","capabilities":["TR"]}),
        json!({"model":"fixture","provider":"other"}),
        json!({"model":"fixture","inference_metadata":{"request_modality":["Image"],"response_modality":["Text"]}}),
    ] {
        let server = Server::start(vec![reply(page(1, 1, vec![malformed]))]).await;
        assert!(server.fetch().await.is_err());
        assert_eq!(server.calls.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn qwen_native_catalog_rejects_redirect_http_errors_and_openai_schema() {
    for response in [
        Response::builder()
            .status(302)
            .header("location", "https://evil.invalid/collect")
            .body(Body::empty())
            .unwrap(),
        Response::builder()
            .status(401)
            .body(Body::from("synthetic-upstream-secret"))
            .unwrap(),
        reply(json!({"data":[{"id":"qwen-contract"}]})),
        Response::builder()
            .body(Body::from("not-json synthetic-upstream-secret"))
            .unwrap(),
    ] {
        let server = Server::start(vec![response]).await;
        let error = server.fetch().await.unwrap_err();
        assert!(!error.to_string().contains("synthetic-upstream-secret"));
        assert_eq!(server.calls.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn qwen_catalog_enforces_per_page_and_total_decoded_byte_limits() {
    let chunk = Bytes::from(vec![b'x'; LIMIT / 2 + 1]);
    let stream = futures::stream::iter(vec![Ok::<_, std::io::Error>(chunk.clone()), Ok(chunk)]);
    let response = Response::builder().body(Body::from_stream(stream)).unwrap();
    let server = Server::start(vec![response]).await;
    assert!(server.fetch().await.is_err());
    assert_eq!(server.calls.lock().unwrap().len(), 1);
    let replies = (1..=5)
        .map(|number| {
            let mut body = page(
                number,
                100,
                ((number - 1) * 20..number * 20).map(row).collect(),
            );
            body["padding"] = json!("x".repeat(1800 * 1024));
            reply(body)
        })
        .collect();
    let server = Server::start(replies).await;
    assert!(server.fetch().await.is_err());
    assert_eq!(server.calls.lock().unwrap().len(), 5);
}
