//! A shared transport must not share cookies across account-local request chains.
use super::*;
use axum::{body::Body, http::Response, routing::get};
use futures::StreamExt;

struct LocalServer {
    url: String,
    task: tokio::task::JoinHandle<()>,
}

impl LocalServer {
    async fn start(app: Router) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self { url, task }
    }
}

impl Drop for LocalServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn run(http: &Client, base: &str, account: &str, streaming: bool) {
    let mut payload = make_payload(base);
    payload.extra_body = None;
    payload.api_key = account.into();
    payload
        .headers
        .insert("Cookie".into(), format!("sid=original-{account}"));
    let extra = HashMap::from([("Cookie".into(), "sid=must-not-reappear".into())]);
    let mut request = make_request();
    request.stream = streaming;
    if streaming {
        let UpstreamStreamingResponse::NativeEvents(mut stream) = execute_stream(
            http,
            Duration::from_secs(5),
            &payload,
            &request,
            "gpt-5.6-sol",
            Some(&extra),
        )
        .await
        .unwrap() else {
            panic!("byte stream expected")
        };
        while let Some(chunk) = stream.next().await {
            chunk.unwrap();
        }
    } else {
        assert_eq!(
            execute(
                http,
                Duration::from_secs(5),
                &payload,
                &request,
                "gpt-5.6-sol",
                Some(&extra)
            )
            .await
            .unwrap()
            .text,
            "Paris"
        );
    }
}

#[tokio::test]
async fn bootstrap_and_requirements_cookies_remain_in_the_same_account_chain() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let captured = seen.clone();
    let app = Router::new().fallback(move |uri: axum::http::Uri, headers: AxumHeaderMap| {
        let seen = captured.clone();
        async move {
            let account = headers["authorization"].to_str().unwrap().strip_prefix("Bearer ").unwrap().to_string();
            seen.lock().unwrap().push((account.clone(), uri.path().to_string(), headers));
            let (content_type, body, phase) = match uri.path() {
                "/" => ("text/html", "<html><script src=\"https://chatgpt.com/c/foo/_/bar.js\"></script></html>", "bootstrap"),
                "/backend-api/sentinel/chat-requirements/prepare" => ("application/json", r#"{"prepare_token":"fixture-prepare"}"#, "prepared"),
                "/backend-api/sentinel/chat-requirements/finalize" => ("application/json", r#"{"token":"fixture-requirements"}"#, "requirements"),
                "/backend-api/conversation" => ("text/event-stream", "data: {\"p\":\"/message/content/parts/0\",\"o\":\"append\",\"v\":\"Paris\"}\n\ndata: [DONE]\n\n", "conversation"),
                _ => panic!("unexpected fixture path"),
            };
            Response::builder().header("content-type", content_type)
                .header("set-cookie", format!("sid={phase}-{account}; Path=/; HttpOnly"))
                .body(Body::from(body)).unwrap()
        }
    });
    let server = LocalServer::start(app).await;
    let client = Client::builder().no_proxy().build().unwrap();
    for streaming in [false, true] {
        seen.lock().unwrap().clear();
        tokio::join!(
            run(&client, &server.url, "fixture-A", streaming),
            run(&client, &server.url, "fixture-B", streaming)
        );
        // A fresh call reimports only the saved material, not previous transient updates.
        run(&client, &server.url, "fixture-A", streaming).await;
        let seen = seen.lock().unwrap();
        for account in ["fixture-A", "fixture-B"] {
            let items = seen
                .iter()
                .filter(|(name, _, _)| name == account)
                .collect::<Vec<_>>();
            for chain in items.chunks(4) {
                assert_eq!(chain.len(), 4);
                for (index, phase) in ["original", "bootstrap", "prepared", "requirements"]
                    .iter()
                    .enumerate()
                {
                    assert_eq!(
                        chain[index].2["cookie"].to_str().unwrap(),
                        format!("sid={phase}-{account}")
                    );
                }
                assert_eq!(chain[0].1, "/");
                assert_eq!(
                    chain[1].1,
                    "/backend-api/sentinel/chat-requirements/prepare"
                );
                assert_eq!(
                    chain[2].1,
                    "/backend-api/sentinel/chat-requirements/finalize"
                );
                assert_eq!(chain[3].1, "/backend-api/conversation");
                assert_eq!(chain[1].2["oai-device-id"], chain[3].2["oai-device-id"]);
            }
        }
    }
}

#[tokio::test]
async fn bootstrap_redirect_is_not_followed_with_account_material() {
    let hits = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = hits.clone();
    let target = LocalServer::start(Router::new().fallback(move || {
        count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        async { "must not receive credentials" }
    }))
    .await;
    let destination = target.url.clone();
    let source = LocalServer::start(Router::new().route(
        "/",
        get(move || {
            let location = destination.clone();
            async move {
                (
                    axum::http::StatusCode::FOUND,
                    [("location", location)],
                    "private-redirect-body",
                )
            }
        }),
    ))
    .await;
    let mut payload = make_payload(&source.url);
    payload.extra_body = None;
    let client = Client::builder().no_proxy().build().unwrap();
    let error = execute(
        &client,
        Duration::from_secs(5),
        &payload,
        &make_request(),
        "auto",
        None,
    )
    .await
    .unwrap_err();
    assert!(error.message.contains("stage=bootstrap"));
    assert!(!error.message.contains("private-redirect-body"));
    assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 0);
}
