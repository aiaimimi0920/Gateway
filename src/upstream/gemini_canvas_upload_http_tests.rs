use crate::protocol::upstream_body::MAX_ACCUMULATED_UPSTREAM_BODY_BYTES;
use crate::protocol::{gemini_canvas, gemini_web};
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::gemini_canvas_upload_http::upload_gemini_canvas_image_edit_inputs_with_http;
use std::collections::BTreeMap;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Case {
    Success,
    StartHttpError,
    StartMissingUrl,
    FinalizeHttpError,
    FinalizeMissingPath,
    StartBodyTooLarge,
    FinalizeBodyTooLarge,
}

impl Case {
    fn start_only(self) -> bool {
        matches!(
            self,
            Self::StartHttpError | Self::StartMissingUrl | Self::StartBodyTooLarge
        )
    }
}

struct Request {
    line: String,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
}

struct Server(tokio::task::JoinHandle<Vec<Request>>);

impl Drop for Server {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn read_request(socket: &mut TcpStream) -> Request {
    let mut bytes = Vec::new();
    loop {
        let mut chunk = [0; 1024];
        let remaining = 8192usize.checked_sub(bytes.len()).unwrap();
        assert!(remaining > 0, "fixture request exceeded its bound");
        let limit = remaining.min(chunk.len());
        let read = socket.read(&mut chunk[..limit]).await.unwrap();
        assert!(read > 0, "incomplete fixture request");
        bytes.extend_from_slice(&chunk[..read]);
        let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") else {
            continue;
        };
        let mut lines = std::str::from_utf8(&bytes[..end]).unwrap().lines();
        let line = lines.next().unwrap().to_string();
        let headers: BTreeMap<_, _> = lines
            .map(|line| {
                let (key, value) = line.split_once(':').unwrap();
                (key.to_ascii_lowercase(), value.trim().to_string())
            })
            .collect();
        let length: usize = headers["content-length"].parse().unwrap();
        assert!(length <= 8192 - end - 4);
        if bytes.len() == end + 4 + length {
            return Request {
                line,
                headers,
                body: bytes[end + 4..].to_vec(),
            };
        }
        assert!(bytes.len() < end + 4 + length);
    }
}

async fn serve(listener: TcpListener, base: String, case: Case) -> Vec<Request> {
    let start_status = if case == Case::StartHttpError {
        500
    } else {
        200
    };
    let upload_url = (case != Case::StartMissingUrl).then(|| format!("{base}/finalize"));
    let mut responses = vec![(start_status, upload_url, "start-ok")];
    if !case.start_only() {
        let (status, body) = match case {
            Case::FinalizeHttpError => (500, "finalize-failed"),
            Case::FinalizeMissingPath => (200, "not-a-resource"),
            _ => (200, "/contrib_service/ttl_1d/fixture"),
        };
        responses.push((status, None, body));
    }
    let mut requests = Vec::new();
    for (index, (status, upload_url, body)) in responses.into_iter().enumerate() {
        let (mut socket, _) = listener.accept().await.unwrap();
        requests.push(read_request(&mut socket).await);
        let upload_header = upload_url
            .map(|url| format!("x-goog-upload-url: {url}\r\n"))
            .unwrap_or_default();
        let declared_length = if matches!(
            (case, index),
            (Case::StartBodyTooLarge, 0) | (Case::FinalizeBodyTooLarge, 1)
        ) {
            MAX_ACCUMULATED_UPSTREAM_BODY_BYTES + 1
        } else {
            body.len()
        };
        let response = format!(
            "HTTP/1.1 {status} Fixture\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n{upload_header}Connection: close\r\n\r\n{body}",
            declared_length
        );
        socket.write_all(response.as_bytes()).await.unwrap();
    }
    requests
}

fn assert_requests(requests: &[Request], base: &str, case: Case) {
    let start_only = case.start_only();
    assert_eq!(requests.len(), if start_only { 1 } else { 2 });
    for request in requests {
        assert_eq!(request.headers["origin"], base);
        assert_eq!(request.headers["referer"], format!("{base}/"));
        assert_eq!(request.headers["push-id"], "fixture-push");
        assert_eq!(request.headers["x-client-pctx"], "fixture-context");
        assert_eq!(request.headers["x-tenant-id"], "bard-storage");
        assert_eq!(request.headers["cookie"], "SID=fixture; SAPISID=fixture");
    }
    let start = &requests[0];
    assert_eq!(start.line, "POST /upload/ HTTP/1.1");
    assert_eq!(start.headers["x-goog-upload-command"], "start");
    assert_eq!(start.headers["x-goog-upload-protocol"], "resumable");
    assert_eq!(start.headers["x-goog-upload-header-content-length"], "5");
    assert_eq!(
        start.headers["content-type"],
        "application/x-www-form-urlencoded;charset=UTF-8"
    );
    assert_eq!(start.body, b"File name: fixture.png");
    if let Some(finalize) = requests.get(1) {
        assert_eq!(finalize.line, "POST /finalize HTTP/1.1");
        assert_eq!(
            finalize.headers["x-goog-upload-command"],
            "upload, finalize"
        );
        assert_eq!(finalize.headers["x-goog-upload-offset"], "0");
        assert_eq!(
            finalize.headers["content-type"],
            "application/x-www-form-urlencoded;charset=utf-8"
        );
        assert_eq!(finalize.body, [0, 1, 128, 255, 3]);
    }
}

fn assert_error(error: crate::error::GatewayError, base: &str, case: Case) {
    assert_eq!(
        error.provider_name.as_deref(),
        Some("gemini_canvas_compatible")
    );
    if matches!(case, Case::StartBodyTooLarge | Case::FinalizeBodyTooLarge) {
        assert_eq!(error.code.as_deref(), Some("upstream_body_too_large"));
        let phase = if case == Case::StartBodyTooLarge {
            "start"
        } else {
            "finalize"
        };
        assert_eq!(error.message, format!(
            "Gemini Canvas upload {phase} response exceeded the {MAX_ACCUMULATED_UPSTREAM_BODY_BYTES}-byte accumulation limit."
        ));
        return;
    }
    let (request, response) = if matches!(case, Case::StartHttpError | Case::StartMissingUrl) {
        let present = case != Case::StartMissingUrl;
        (
            format!("upload_base_url={base}/upload/, file_name=fixture.png, mime_type=image/png, bytes=5, push_id=fixture-push, client_pctx=fixture-context, referer={base}/"),
            format!("final_url={base}/upload/, content_type=text/plain, upload_url_present={present}, body_preview=start-ok"),
        )
    } else {
        let body = if case == Case::FinalizeHttpError {
            "finalize-failed"
        } else {
            "not-a-resource"
        };
        (
            format!(
                "upload_url={base}/finalize, file_name=fixture.png, mime_type=image/png, bytes=5"
            ),
            format!("final_url={base}/finalize, content_type=text/plain, body_preview={body}"),
        )
    };
    assert!(
        error.message.ends_with(&format!(
            "; upload_request_contract={request}; upload_response_meta={response}"
        )),
        "unexpected error contract for {case:?}: {}",
        error.message
    );
    match case {
        Case::StartMissingUrl => assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_image_edit_missing_upload_url")
        ),
        Case::FinalizeMissingPath => assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_image_edit_missing_resource_path")
        ),
        _ => assert_eq!(error.http_status, Some(500)),
    }
}

async fn run_case(case: Case) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let mut server = Server(tokio::spawn(serve(listener, base.clone(), case)));
    let payload: ProviderAccountPayload = serde_json::from_value(serde_json::json!({
        "adapter": "gemini_canvas_compatible", "baseUrl": base, "apiKey": "fixture-key",
        "extraBody": {"imageEditUploadBaseUrl": format!("{base}/upload/")}
    }))
    .unwrap();
    let session = gemini_canvas::GeminiCanvasPureHttpSession {
        cookie_header: "SID=fixture; SAPISID=fixture".into(),
        sapisid: "fixture".into(),
        auth_user: "1".into(),
    };
    let bootstrap = gemini_web::GeminiWebBootstrap {
        access_token: None,
        build_label: None,
        session_id: None,
        language: "en-US".into(),
        push_id: Some("fixture-push".into()),
        client_pctx: Some("fixture-context".into()),
        app_page_path: Some("/app".into()),
    };
    let upload = gemini_canvas::GeminiCanvasImageEditUpload {
        mime_type: "image/png".into(),
        bytes: vec![0, 1, 128, 255, 3],
        file_name: "fixture.png".into(),
        source_mime_type: "image/png".into(),
        source_bytes: Vec::new(),
    };
    let http = rquest::Client::builder().no_proxy().build().unwrap();
    let result = upload_gemini_canvas_image_edit_inputs_with_http(
        &http,
        &payload,
        &session,
        &bootstrap,
        &[upload],
        Duration::from_secs(1),
    )
    .await;
    let requests = (&mut server.0).await.unwrap();
    assert_requests(&requests, &base, case);
    if case == Case::Success {
        let refs = result.unwrap();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].resource_path, "/contrib_service/ttl_1d/fixture");
        assert_eq!(refs[0].mime_type, "image/png");
        assert_eq!(refs[0].file_name, "fixture.png");
    } else {
        assert_error(result.unwrap_err(), &base, case);
    }
}

pub(super) fn run_http_contracts(trace_enabled: bool) {
    // The parent gives this child an exclusive empty cwd; no browser encoder can be discovered.
    for script in [
        "scripts/gemini-canvas-image-edit-encode.mjs",
        "gateway/scripts/gemini-canvas-image-edit-encode.mjs",
    ] {
        assert!(!std::path::Path::new(script).exists());
    }
    assert_eq!(
        std::env::var_os("GEMINI_CANVAS_IMAGE_EDIT_TRACE").is_some(),
        trace_enabled
    );
    assert!(std::env::var_os("GEMINI_CANVAS_IMAGE_EDIT_RESOURCE_PATH_OVERRIDE").is_none());
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            for case in [
                Case::Success,
                Case::StartHttpError,
                Case::StartMissingUrl,
                Case::FinalizeHttpError,
                Case::FinalizeMissingPath,
                Case::StartBodyTooLarge,
                Case::FinalizeBodyTooLarge,
            ] {
                tokio::time::timeout(Duration::from_secs(5), run_case(case))
                    .await
                    .unwrap_or_else(|_| panic!("upload fixture timed out: {case:?}"));
            }
        });
}
