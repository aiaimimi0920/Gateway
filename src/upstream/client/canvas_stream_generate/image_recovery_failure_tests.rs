//! Error, admission and cancellation contracts for the same read-only owner.
use super::*;
use crate::protocol::gemini_canvas::image_recovery::IMAGE_READ_LIMIT;
use axum::{
    body::Body,
    http::{Response, StatusCode},
};
use futures::stream;

const HTML: &str = r#"<script>window.WIZ_global_data={"SNlM0e":"fixture-xsrf","cfb2h":"fixture-build","FdrFJe":"fixture-session"};</script>"#;

#[derive(Clone, Copy)]
enum Case {
    BootstrapRedirect,
    BootstrapInvalid,
    ReadRedirect,
    ReadError,
    ReadLarge,
    ReadChunked,
    Pending,
    WrongCandidate,
    InvalidBody,
}

async fn exercise(
    case: Case,
    primary: &str,
    budget: Duration,
) -> (
    Result<Vec<gemini_canvas::GeminiCanvasMediaAsset>, GatewayError>,
    usize,
    usize,
) {
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
                    assert_eq!(headers.get("cookie").unwrap(), "fixture=session");
                    assert!(headers.get("authorization").is_none());
                    if matches!(case, Case::BootstrapRedirect) {
                        Response::builder()
                            .status(StatusCode::TEMPORARY_REDIRECT)
                            .header("location", "/app")
                            .body(Body::empty())
                            .unwrap()
                    } else {
                        Response::new(Body::from(if matches!(case, Case::BootstrapInvalid) {
                            "private fixture missing token"
                        } else {
                            HTML
                        }))
                    }
                }
            }),
        )
        .route(
            "/_/BardChatUi/data/batchexecute",
            post(
                move |Query(query): Query<HashMap<String, String>>,
                      headers: axum::http::HeaderMap,
                      Form(form): Form<HashMap<String, String>>| {
                    read_count.fetch_add(1, Ordering::SeqCst);
                    async move {
                        assert_eq!(query["rpcids"], "hNvQHb");
                        assert_eq!(headers.get("cookie").unwrap(), "fixture=session");
                        assert_eq!(form.len(), 2);
                        let (status, body) = match case {
                            Case::ReadRedirect => (
                                StatusCode::TEMPORARY_REDIRECT,
                                "private fixture redirect".into(),
                            ),
                            Case::ReadError => (
                                StatusCode::BAD_GATEWAY,
                                "private fixture cookie=secret".into(),
                            ),
                            Case::ReadLarge => (StatusCode::OK, "x".repeat(IMAGE_READ_LIMIT + 1)),
                            Case::InvalidBody => (
                                StatusCode::OK,
                                "private fixture c_fixture r_fixture rc_fixture".into(),
                            ),
                            Case::WrongCandidate => (
                                StatusCode::OK,
                                history(true).replace("rc_fixture", "rc_other"),
                            ),
                            _ => (StatusCode::OK, history(false)),
                        };
                        if matches!(case, Case::ReadChunked) {
                            let chunks = vec![
                                Ok::<_, std::io::Error>(vec![b'x'; IMAGE_READ_LIMIT]),
                                Ok(vec![b'x'; 1]),
                            ];
                            Response::new(Body::from_stream(stream::iter(chunks)))
                        } else {
                            Response::builder()
                                .status(status)
                                .header("location", "/_/BardChatUi/data/batchexecute")
                                .body(Body::from(body))
                                .unwrap()
                        }
                    }
                },
            ),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let mut client = UpstreamClient::new(1);
    client.http = rquest::Client::builder().no_proxy().build().unwrap();
    let result = client
        .resolve_gemini_web_image_assets(
            &payload(&base),
            &prepared(&base).session,
            None,
            primary,
            budget,
        )
        .await;
    let counts = (gets.load(Ordering::SeqCst), reads.load(Ordering::SeqCst));
    // A cancelled/deadline-expired owner must not leave a detached read poller.
    if matches!(case, Case::Pending) {
        tokio::time::sleep(Duration::from_millis(2100)).await;
    }
    assert_eq!(
        counts,
        (gets.load(Ordering::SeqCst), reads.load(Ordering::SeqCst))
    );
    server.abort();
    let _ = server.await;
    (result, counts.0, counts.1)
}

#[tokio::test]
async fn missing_binding_never_bootstraps_or_reads() {
    let (result, gets, reads) = exercise(
        Case::Pending,
        "c_fixture r_fixture rc_fixture",
        Duration::from_secs(2),
    )
    .await;
    assert!(result.is_err());
    assert_eq!((gets, reads), (0, 0));
}

#[tokio::test]
async fn read_failures_stop_without_refresh_redirect_or_private_error_text() {
    for (case, expected_reads) in [
        (Case::BootstrapRedirect, 0),
        (Case::BootstrapInvalid, 0),
        (Case::ReadRedirect, 1),
        (Case::ReadError, 1),
        (Case::ReadLarge, 1),
        (Case::ReadChunked, 1),
        (Case::InvalidBody, 1),
    ] {
        let (result, gets, reads) = exercise(case, &accepted(), Duration::from_secs(3)).await;
        let error = result.err().expect("unsafe result must fail");
        assert_eq!((gets, reads), (1, expected_reads));
        assert!(!error.message.contains("fixture"));
        assert!(!error.message.contains("secret"));
        assert!(!error.retryable);
        assert!(matches!(
            error.fallback_hint,
            crate::error::FallbackHint::Abort { .. }
        ));
    }
}

#[tokio::test]
async fn pending_and_wrong_candidate_are_not_success_and_share_one_deadline() {
    for case in [Case::Pending, Case::WrongCandidate] {
        let (result, gets, reads) = exercise(case, &accepted(), Duration::from_millis(100)).await;
        let error = result.err().unwrap();
        assert_eq!(error.code.as_deref(), Some("gemini_web_image_read_timeout"));
        assert_eq!((gets, reads), (1, 1));
    }
}

#[tokio::test]
async fn pending_reads_have_a_fixed_attempt_cap() {
    let (result, gets, reads) = exercise(Case::Pending, &accepted(), Duration::from_secs(15)).await;
    assert_eq!(
        result.err().unwrap().code.as_deref(),
        Some("gemini_web_image_read_pending")
    );
    assert_eq!((gets, reads), (1, 6));
}

#[tokio::test]
async fn complete_primary_image_returns_without_any_recovery_io() {
    let body = json!([[
        "wrb.fr",
        null,
        json!([
            null,
            ["c_fixture", "r_fixture"],
            null,
            null,
            [candidate("rc_fixture", true)]
        ])
        .to_string()
    ]])
    .to_string();
    let body = format!(")]}}'\n{}\n{}\n", body.encode_utf16().count(), body);
    let (result, gets, reads) = exercise(Case::InvalidBody, &body, Duration::from_secs(2)).await;
    assert!(result.is_ok());
    assert_eq!((gets, reads), (0, 0));
}
