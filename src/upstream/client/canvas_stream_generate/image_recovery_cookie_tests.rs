//! Read-only recovery must retain scoped rotations without changing the imported seed.
use super::*;
use axum::{body::Body, http::Response};

const HTML: &str = r#"<script>window.WIZ_global_data={"SNlM0e":"fixture-xsrf","cfb2h":"fixture-build","FdrFJe":"fixture-session"};</script>"#;

#[derive(Clone, Copy)]
enum Case {
    BootstrapRotation,
    PollRotation,
    Deletion,
    Scope,
}

async fn exercise(case: Case) {
    let gets = Arc::new(AtomicUsize::new(0));
    let reads = Arc::new(AtomicUsize::new(0));
    let get_count = gets.clone();
    let read_count = reads.clone();
    let app = Router::new()
        .route(
            "/app",
            get(move |headers: axum::http::HeaderMap| {
                get_count.fetch_add(1, Ordering::SeqCst);
                async move {
                    // Each new recovery still begins with the unchanged caller-owned seed.
                    if headers.get("cookie").and_then(|v| v.to_str().ok())
                        != Some("fixture=session")
                    {
                        return Response::builder().status(401).body(Body::empty()).unwrap();
                    }
                    let response = Response::builder();
                    let response = match case {
                        Case::BootstrapRotation => {
                            response.header("set-cookie", "fixture=boot; Path=/")
                        }
                        Case::PollRotation => response,
                        Case::Deletion => {
                            response.header("set-cookie", "fixture=; Path=/; Max-Age=0")
                        }
                        Case::Scope => response
                            .header("set-cookie", "recovery=valid; Path=/_/")
                            .header("set-cookie", "app_only=private; Path=/app")
                            .header(
                                "set-cookie",
                                "foreign=private; Domain=example.invalid; Path=/",
                            ),
                    };
                    response.body(Body::from(HTML)).unwrap()
                }
            }),
        )
        .route(
            "/_/BardChatUi/data/batchexecute",
            post(move |headers: axum::http::HeaderMap| {
                let count = read_count.fetch_add(1, Ordering::SeqCst);
                async move {
                    let cookie = headers
                        .get("cookie")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("");
                    let expected = match case {
                        Case::BootstrapRotation => cookie == "fixture=boot",
                        Case::PollRotation if count % 2 == 0 => cookie == "fixture=session",
                        Case::PollRotation => cookie == "fixture=poll",
                        Case::Deletion => cookie.is_empty(),
                        Case::Scope => cookie == "recovery=valid; fixture=session",
                    };
                    if !expected || headers.contains_key("authorization") {
                        return Response::builder().status(401).body(Body::empty()).unwrap();
                    }
                    if matches!(case, Case::PollRotation) && count % 2 == 0 {
                        Response::builder()
                            .header("set-cookie", "fixture=poll; Path=/")
                            .body(Body::from(history(false)))
                            .unwrap()
                    } else {
                        Response::new(Body::from(history(true)))
                    }
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let mut client = UpstreamClient::new(1);
    client.http = rquest::Client::builder().no_proxy().build().unwrap();
    let mut results = Vec::new();
    // Reuse the HTTP client, not the request-local authentication state.
    for _ in 0..2 {
        results.push(
            client
                .resolve_gemini_web_image_assets(
                    &payload(&base),
                    &prepared(&base).session,
                    None,
                    &accepted(),
                    Duration::from_secs(5),
                )
                .await,
        );
    }
    server.abort();
    let _ = server.await;
    for result in results {
        assert!(
            result.is_ok(),
            "Cookie-aware readback must succeed: {:?}",
            result.err()
        );
    }
    assert_eq!(gets.load(Ordering::SeqCst), 2);
    assert_eq!(
        reads.load(Ordering::SeqCst),
        if matches!(case, Case::PollRotation) {
            4
        } else {
            2
        }
    );
}

#[tokio::test]
async fn image_readback_uses_bootstrap_cookie_rotation() {
    exercise(Case::BootstrapRotation).await;
}

#[tokio::test]
async fn image_readback_uses_pending_poll_cookie_rotation() {
    exercise(Case::PollRotation).await;
}

#[tokio::test]
async fn image_readback_does_not_resurrect_a_deleted_seed() {
    exercise(Case::Deletion).await;
}

#[tokio::test]
async fn image_readback_honors_cookie_paths_and_rejects_foreign_domains() {
    exercise(Case::Scope).await;
}
