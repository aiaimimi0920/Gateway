//! Opt-in normal Web UI host. Remapping is test-only; every upstream call is verified at shutdown.
use super::{files_tests::metadata, pipeline_fixture::*, test_support::*};
use axum::{body::Body, extract::Request, middleware::Next};
use serde_json::{json, Value};
use std::{net::SocketAddr, path::PathBuf, time::Duration};

const LARGE_ID: &str = "d290f1ee-6c54-4b01-90e6-d701748f0852";
const LARGE_SIZE: usize = 8 * 1024 * 1024 + 1;

fn large(state: &str) -> Value {
    json!({"id":LARGE_ID,"filename":"large.png","mime_type":"image/png",
        "size_bytes":LARGE_SIZE,"purpose":"reference","state":state,
        "created_at":"2026-11-20T00:00:00Z","expires_at":"2099-01-01T00:00:00Z",
        "user_id":"owner-tag"})
}
fn replies() -> Vec<Reply> {
    vec![
        Reply::json(
            201,
            json!({"id":ID,"state":"pending","upload":null,"file":metadata("pending")}),
        ),
        Reply::json(200, metadata("ready")),
        Reply::json(
            200,
            json!({"data":[metadata("ready")],"has_more":true,"next_cursor":"next+/="}),
        ),
        Reply::json(200, json!({"data":[metadata("ready")],"has_more":false})),
        Reply::json(
            201,
            json!({"id":LARGE_ID,"state":"pending","file":large("pending"),
            "upload":{"url":ASSET,"method":"PUT","expires_at":"2099-01-01T00:00:00Z",
            "headers":{"Content-Type":"image/png"}}}),
        ),
        Reply::json(200, Value::Null),
        Reply::json(200, large("pending")),
        Reply::json(200, large("ready")),
        Reply::json(204, Value::Null),
    ]
}

#[tokio::test]
#[ignore = "Requires tools/luma/files-browser-check.mjs and an explicit new fixture root"]
async fn luma_agents_files_normal_main_browser_host() {
    tokio::time::timeout(Duration::from_secs(1800), Box::pin(serve()))
        .await
        .unwrap();
}
async fn serve() {
    let root = PathBuf::from(
        std::env::var_os("GATEWAY_LUMA_FILES_UI_ROOT").expect("Explicit fixture root required"),
    );
    assert!(
        root.is_absolute() && !root.exists(),
        "Use a new absolute fixture root"
    );
    std::fs::create_dir(&root).unwrap();
    let server = Server::start(replies()).await;
    let fixture = Box::pin(Fixture::new(&server, 5)).await;
    let revision = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_owned();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let remap = server.origin.clone();
    let app = crate::http::router::build_router(fixture.state.clone()).layer(
        axum::middleware::from_fn(move |request: Request, next: Next| {
            let origin = remap.clone();
            async move { within(&origin, next.run(request)).await }
        }),
    );
    let (shutdown, stopped) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(async {
            let _ = stopped.await;
        })
        .await
        .unwrap();
    });
    std::fs::write(
        root.join("ready.json"),
        serde_json::to_vec_pretty(&json!({
            "origin":format!("http://{address}"),"pid":std::process::id(),"storageMode":"local",
            "provider":"luma-pipeline","credential":"exact","syntheticOnly":true,
            "scenario":"luma-files-normal-main-v1","revision":revision
        }))
        .unwrap(),
    )
    .unwrap();
    let success = loop {
        tokio::time::sleep(Duration::from_millis(200)).await;
        match std::fs::read(root.join("command.json")) {
            Ok(bytes) => {
                let command: Value = serde_json::from_slice(&bytes).unwrap();
                assert!(matches!(
                    command["action"].as_str(),
                    Some("finish" | "abort")
                ));
                break command["action"] == "finish";
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => panic!("Fixture command read failed: {error}"),
        }
    };
    shutdown.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .unwrap()
        .unwrap();
    let calls = server.calls();
    // Always release TCP/runtime owners, including browser failures, before checking acceptance.
    let balance = fixture.balance().await.remaining_messages;
    let final_revision = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_owned();
    fixture.finish().await;
    server.stop().await;
    assert!(success, "Browser did not complete; isolated host shut down");
    assert_eq!(balance, Some(2));
    assert_eq!(revision, final_revision);
    verify(&calls).await;
    std::fs::write(root.join("completed.json"), serde_json::to_vec_pretty(&json!({
        "status":"passed","ordinaryKeyBalance":balance,"revisionUnchanged":true,
        "upstreamCalls":calls.iter().map(|c|json!({"method":c.method,
            "path":c.path.split('?').next().unwrap(),"bytes":c.raw_body.len()})).collect::<Vec<_>>(),
        "scope":"normal main to management TCP and synthetic upstream; no real vendor or charge"
    })).unwrap()).unwrap();
}

async fn verify(calls: &[Call]) {
    use axum::extract::{FromRequest, Multipart};
    assert_eq!(calls.len(), 9);
    for (i, (method, path)) in [
        ("POST", "/v1/files".to_owned()),
        ("GET", format!("/v1/files/{ID}")),
        ("GET", "/v1/files?limit=25".to_owned()),
        ("GET", "/v1/files?limit=25&cursor=next%2B%2F%3D".to_owned()),
        ("POST", "/v1/files".to_owned()),
        ("PUT", "/output.png?signature=private".to_owned()),
        ("POST", format!("/v1/files/{LARGE_ID}/complete")),
        ("GET", format!("/v1/files/{LARGE_ID}")),
        ("DELETE", format!("/v1/files/{LARGE_ID}")),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(calls[i].method, method);
        assert_eq!(calls[i].path, path);
        for name in ["cookie", "origin", "x-secret-grant", "x-management-token"] {
            assert!(!calls[i].headers.contains_key(name));
        }
        if method == "PUT" {
            assert!(!calls[i].headers.contains_key("authorization"));
        } else {
            assert_eq!(calls[i].headers["authorization"], format!("Bearer {KEY}"));
        }
    }
    let mut request = Request::new(Body::from(calls[0].raw_body.clone()));
    *request.headers_mut() = calls[0].headers.clone();
    let mut form = Multipart::from_request(request, &()).await.unwrap();
    assert_eq!(
        form.next_field()
            .await
            .unwrap()
            .unwrap()
            .text()
            .await
            .unwrap(),
        "reference"
    );
    let file = form.next_field().await.unwrap().unwrap();
    assert_eq!(file.file_name(), Some("参考.png"));
    assert_eq!(file.bytes().await.unwrap().as_ref(), &[1, 2, 3]);
    assert!(form.next_field().await.unwrap().is_none());
    assert_eq!(
        calls[4].body,
        json!({"filename":"large.png","mime_type":"image/png",
        "size_bytes":LARGE_SIZE,"purpose":"reference","expires_at":"2099-01-01T00:00:00.000Z","user_id":"owner-tag"})
    );
    assert_eq!(calls[5].raw_body.len(), LARGE_SIZE);
    assert!(calls[5].raw_body.iter().all(|b| *b == 7));
}
