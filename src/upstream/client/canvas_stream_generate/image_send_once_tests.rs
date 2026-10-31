//! The no-template path also performs exactly one generation POST.
use super::*;

pub(super) fn context(base: &str, edit: bool) -> StreamGenerateRequestContext {
    let mut preparation = prepared(base);
    preparation.replay_template = None;
    preparation.is_image_edit_request = edit;
    StreamGenerateRequestContext {
        preparation,
        bootstrap: gemini_web::GeminiWebBootstrap {
            access_token: Some("old-fixture-xsrf".into()),
            build_label: None,
            session_id: None,
            language: "en".into(),
            push_id: None,
            client_pctx: None,
            app_page_path: Some("/app".into()),
        },
        origin: base.into(),
        authorization: "fixture".into(),
        page_path: "/app".into(),
        text_preflight_source_path: "/app".into(),
        referer: format!("{base}/app"),
        model_header: "[]".into(),
        request_uuid: "fixture".into(),
        request: gemini_web::GeminiWebRequest {
            query: vec![],
            form: vec![("at".into(), "old-fixture-xsrf".into())],
        },
    }
}

#[tokio::test]
async fn heavy_image_and_edit_sends_never_retry_xsrf_or_fall_back_to_browser_fetch() {
    for edit in [false, true] {
        for status in [200, 400, 403, 502, 307, 308] {
            let hits = Arc::new(AtomicUsize::new(0));
            let observed = hits.clone();
            let app = Router::new().route(
                gemini_web::GEMINI_WEB_DEFAULT_STREAM_GENERATE_PATH,
                post(move || {
                    observed.fetch_add(1, Ordering::SeqCst);
                    async move {
                        (
                            axum::http::StatusCode::from_u16(status).unwrap(),
                            [(
                                "location",
                                gemini_web::GEMINI_WEB_DEFAULT_STREAM_GENERATE_PATH,
                            )],
                            r#")]}' {"at":"new-fixture-xsrf"}"#,
                        )
                    }
                }),
            );
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
            let runtime = gemini_canvas::GeminiCanvasRuntime {
                share_id: "fixture".into(),
                runtime_state_object_key: "not-read".into(),
                api_base_url: base.clone(),
            };
            let mut client = UpstreamClient::new(1);
            client.http = rquest::Client::builder().no_proxy().build().unwrap();
            let result = client
                .send_gemini_canvas_stream_generate_request(
                    context(&base, edit),
                    &payload(&base),
                    "fixture",
                    &runtime,
                    gemini_canvas::GEMINI_CANVAS_STREAM_GENERATE_IMAGE_MODE_INDEX,
                    Duration::from_secs(2),
                    false,
                    &mut None,
                )
                .await;
            server.abort();
            let _ = server.await;
            assert_eq!(
                hits.load(Ordering::SeqCst),
                1,
                "edit={edit}, status={status}"
            );
            assert_eq!(result.is_ok(), status == 200);
        }
    }
}
