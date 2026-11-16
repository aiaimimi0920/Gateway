//! Final transport must not redirect a Kiro bearer; this tests transport policy, not AWS reachability.
use super::*;
use axum::{body::Body, response::Response, routing::post, Router};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

struct Peer {
    origin: String,
    redirected: Arc<AtomicUsize>,
    task: Option<tokio::task::JoinHandle<()>>,
}
impl Peer {
    async fn new() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let destination = format!("{origin}/redirected");
        let redirected = Arc::new(AtomicUsize::new(0));
        let hits = redirected.clone();
        let app = Router::new()
            .route(
                "/start",
                post(move || {
                    let destination = destination.clone();
                    async move {
                        Response::builder()
                            .status(307)
                            .header("location", destination)
                            .body(Body::empty())
                            .unwrap()
                    }
                }),
            )
            .route(
                "/redirected",
                post(move || {
                    let hits = hits.clone();
                    async move {
                        hits.fetch_add(1, Ordering::SeqCst);
                        "must not follow"
                    }
                }),
            );
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self {
            origin,
            redirected,
            task: Some(task),
        }
    }
    async fn stop(mut self) {
        let task = self.task.take().unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}

#[tokio::test]
async fn kiro_final_profiled_sender_does_not_follow_redirect() {
    let peer = Peer::new().await;
    let payload: ProviderAccountPayload = serde_json::from_value(json!({
        "adapter":"kiro_compatible","base_url":"https://q.us-east-1.amazonaws.com",
        "api_key":"synthetic-access","session_auth":{"transport":"bearer"}}))
    .unwrap();
    // Invoke the same final transport owner used by buffered and stream sends, without a live AWS target.
    let plan = RequestPlan {
        method: rquest::Method::POST,
        url: format!("{}/start", peer.origin),
        query: vec![],
        body: Some(json!({"synthetic":true})),
        response_kind: EndpointKind::ChatCompletions,
    };
    let mut client = UpstreamClient::new(3);
    client.http = rquest::Client::builder()
        .no_proxy()
        .redirect(rquest::redirect::Policy::limited(3))
        .build()
        .unwrap();
    let headers = crate::upstream::headers::build_upstream_headers(&payload);
    let response = client
        .send_profiled_plan(&payload, &plan, headers)
        .unwrap()
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 307);
    assert_eq!(peer.redirected.load(Ordering::SeqCst), 0);
    peer.stop().await;
}
