//! Catalog bootstrap is account-local and does not require proof-script parsing.
use super::*;
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn text_compat_catalog_bootstraps_same_session_before_reading_models() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let captured = seen.clone();
    let app = Router::new().fallback(move |uri: axum::http::Uri, headers: HeaderMap| {
        let seen = captured.clone();
        async move {
            seen.lock()
                .unwrap()
                .push((uri.path().to_string(), headers.clone()));
            if uri.path() == "/" {
                axum::response::Response::builder()
                    .header("content-type", "text/html")
                    .header("set-cookie", "session=updated; Path=/; HttpOnly")
                    .body(axum::body::Body::from(
                        "<html>fixture homepage without scripts</html>",
                    ))
                    .unwrap()
            } else {
                axum::response::Response::builder()
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        r#"{"models":[{"slug":"gpt-5.6-sol"}]}"#,
                    ))
                    .unwrap()
            }
        }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let payload: ProviderAccountPayload = serde_json::from_value(json!({
        "adapter":"chatgpt_web_reverse_compatible", "baseUrl":format!("http://{address}"),
        "apiKey":"fixture-token", "headers":{"Cookie":"session=old; keep=value", "User-Agent":"FixtureBrowser", "Chatgpt-Account-Id":"must-not-send", "Host":"attacker.invalid", "Authorization":"Bearer must-not-send", "Proxy-Authorization":"must-not-send", "X-Secret":"must-not-send"},
        "extraBody":{"conversationPath":"https://attacker.invalid", "deviceId":"fixture-device", "languageCode":"zh-CN", "sessionId":"fixture-session", "clientVersion":"fixture-version", "clientBuildNumber":"fixture-build"}
    })).unwrap();
    let client = rquest::Client::builder().no_proxy().build().unwrap();
    let result = fetch_at(&client, &format!("http://{address}/models"), &payload).await;
    task.abort();
    let _ = task.await;
    assert_eq!(result.unwrap()["models"][0]["slug"], "gpt-5.6-sol");
    let seen = seen.lock().unwrap();
    assert_eq!(
        seen.iter()
            .map(|(path, _)| path.as_str())
            .collect::<Vec<_>>(),
        ["/", "/models"]
    );
    for (_, headers) in seen.iter() {
        assert_eq!(headers["authorization"], "Bearer fixture-token");
        assert_eq!(headers["user-agent"], "FixtureBrowser");
        assert!(headers.get("chatgpt-account-id").is_none());
        assert_ne!(headers["host"], "attacker.invalid");
        assert!(headers.get("proxy-authorization").is_none());
        assert!(headers.get("x-secret").is_none());
    }
    assert!(seen[0].1["cookie"]
        .to_str()
        .unwrap()
        .contains("session=old"));
    let cookies = seen[1].1["cookie"].to_str().unwrap();
    assert!(cookies.contains("session=updated"));
    assert!(cookies.contains("keep=value"));
    assert!(!cookies.contains("session=old"));
    assert_eq!(seen[1].1["oai-device-id"], "fixture-device");
    assert_eq!(seen[1].1["oai-session-id"], "fixture-session");
    assert_eq!(seen[1].1["oai-client-version"], "fixture-version");
    assert_eq!(seen[1].1["oai-client-build-number"], "fixture-build");
}

#[tokio::test]
async fn catalog_bootstrap_rejection_stops_before_models_and_does_not_echo_body() {
    for status in [302, 401, 403] {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let captured = seen.clone();
        let app = Router::new().fallback(move |uri: axum::http::Uri| {
            captured.lock().unwrap().push(uri.path().to_string());
            async move {
                axum::response::Response::builder()
                    .status(status)
                    .header("content-type", "text/html")
                    .header("location", "/models")
                    .body(axum::body::Body::from(
                        "<html>private-bootstrap-body</html>",
                    ))
                    .unwrap()
            }
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let payload: ProviderAccountPayload = serde_json::from_value(json!({
            "adapter":"chatgpt_web_reverse_compatible", "baseUrl":base, "apiKey":"fixture-token"
        }))
        .unwrap();
        let client = rquest::Client::builder().no_proxy().build().unwrap();
        let error = fetch_at(&client, &format!("{base}/models"), &payload)
            .await
            .unwrap_err();
        task.abort();
        let _ = task.await;
        assert!(error.message.contains("stage=bootstrap"));
        assert!(!error.message.contains("private-bootstrap-body"));
        assert_eq!(*seen.lock().unwrap(), ["/"]);
    }
}
