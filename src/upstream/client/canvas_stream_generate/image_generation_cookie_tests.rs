//! Generation-to-readback rotations remain scoped to one synthetic request.
use super::*;
use axum::{body::Body, http::Response};

const HTML: &str = r#"<script>window.WIZ_global_data={"SNlM0e":"fixture-xsrf","cfb2h":"fixture-build","FdrFJe":"fixture-session"};</script>"#;

#[derive(Clone, Copy)]
enum Case {
    Rotation,
    Deletion,
    Scope,
}

fn valid_cookie(headers: &axum::http::HeaderMap, case: Case, bootstrap: bool) -> bool {
    let raw = headers
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    match case {
        Case::Rotation => raw == "fixture=generated",
        Case::Deletion => raw.is_empty(),
        Case::Scope if bootstrap => raw == "app_only=private; fixture=generated",
        Case::Scope => raw == "history_only=valid; fixture=generated",
    }
}

async fn exercise(case: Case, heavy_send: bool) {
    let generations = Arc::new(AtomicUsize::new(0));
    let reads = Arc::new(AtomicUsize::new(0));
    let generated = generations.clone();
    let read = reads.clone();
    let app = Router::new()
        .route(
            gemini_web::GEMINI_WEB_DEFAULT_STREAM_GENERATE_PATH,
            post(move |headers: axum::http::HeaderMap| {
                generated.fetch_add(1, Ordering::SeqCst);
                async move {
                    assert_eq!(headers.get("cookie").unwrap(), "fixture=session");
                    let response = Response::builder();
                    let response = match case {
                        Case::Rotation => {
                            response.header("set-cookie", "fixture=generated; Path=/")
                        }
                        Case::Deletion => {
                            response.header("set-cookie", "fixture=; Path=/; Max-Age=0")
                        }
                        Case::Scope => response
                            .header("set-cookie", "fixture=generated; Path=/")
                            .header("set-cookie", "app_only=private; Path=/app")
                            .header(
                                "set-cookie",
                                "history_only=valid; Path=/_/BardChatUi/data/batchexecute",
                            )
                            .header(
                                "set-cookie",
                                "foreign=private; Domain=example.invalid; Path=/",
                            ),
                    };
                    response.body(Body::from(accepted())).unwrap()
                }
            }),
        )
        .route(
            "/app",
            get(move |headers: axum::http::HeaderMap| async move {
                Response::builder()
                    .status(if valid_cookie(&headers, case, true) {
                        200
                    } else {
                        401
                    })
                    .body(Body::from(HTML))
                    .unwrap()
            }),
        )
        .route(
            "/_/BardChatUi/data/batchexecute",
            post(move |headers: axum::http::HeaderMap| {
                read.fetch_add(1, Ordering::SeqCst);
                async move {
                    Response::builder()
                        .status(if valid_cookie(&headers, case, false) {
                            200
                        } else {
                            401
                        })
                        .body(Body::from(history(true)))
                        .unwrap()
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let prepared = prepared(&base);
    let payload = payload(&base);
    let runtime = gemini_canvas::GeminiCanvasRuntime {
        share_id: "fixture".into(),
        runtime_state_object_key: "not-read".into(),
        api_base_url: base.clone(),
    };
    let mut client = UpstreamClient::new(1);
    client.http = rquest::Client::builder().no_proxy().build().unwrap();
    let mut results = vec![];
    // Reusing one HTTP client must not leak rotations/deletion into a second generation.
    for _ in 0..2 {
        let body = if heavy_send {
            client
                .send_gemini_canvas_stream_generate_request(
                    heavy::context(&base, false),
                    &payload,
                    "fixture",
                    &runtime,
                    gemini_canvas::GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX,
                    Duration::from_secs(2),
                    false,
                    &mut None,
                )
                .await
                .unwrap()
        } else {
            client
                .send_gemini_web_image_template_once(
                    &payload,
                    &prepared,
                    prepared.replay_template.as_ref().unwrap(),
                )
                .await
                .unwrap()
        };
        results.push(
            client
                .resolve_gemini_web_image_assets(
                    &payload,
                    &prepared.session,
                    body.cookies,
                    &body.text,
                    Duration::from_secs(2),
                )
                .await,
        );
    }
    server.abort();
    let _ = server.await;
    for result in results {
        assert!(
            result.is_ok(),
            "generation Cookie must reach readback: {:?}",
            result.err()
        );
    }
    assert_eq!(generations.load(Ordering::SeqCst), 2);
    assert_eq!(reads.load(Ordering::SeqCst), 2);
    assert_eq!(prepared.session.cookie_header, "fixture=session");
}

#[tokio::test]
async fn generation_cookie_rotation_reaches_both_readback_senders() {
    for heavy in [false, true] {
        exercise(Case::Rotation, heavy).await;
    }
}

#[tokio::test]
async fn generation_cookie_deletion_is_not_reseeded() {
    for heavy in [false, true] {
        exercise(Case::Deletion, heavy).await;
    }
}

#[tokio::test]
async fn generation_cookie_paths_and_foreign_domain_stay_isolated() {
    for heavy in [false, true] {
        exercise(Case::Scope, heavy).await;
    }
}
