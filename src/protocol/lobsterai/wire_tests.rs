use super::{
    test_support::*,
    transport::{TEST_BUDGET, TEST_ORIGIN},
    *,
};
use serde_json::json;
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

#[tokio::test]
async fn browser_callback_exchange_catalog_and_refresh_are_one_native_chain() {
    let peer = Peer::start(vec![
        Reply::json(tokens()),
        Reply::json(json!({"code":0,"data":[
            {"modelId":"live-model","supportsToolCalling":true,"contextWindow":12345}
        ]})),
        Reply::json(json!({"code":0,"data":{"accessToken":"synthetic-next"}})),
    ])
    .await;
    let login = PendingLogin::bind(peer.client()).await.unwrap();
    let redirect = login.redirect_uri().to_owned();
    let success = callback(&login.authorization_url(), "synthetic-auth-code");
    let browser = async {
        let browser = rquest::Client::builder().no_proxy().build().unwrap();
        let wrong = browser
            .get(format!("{redirect}?state=wrong&code=spoofed"))
            .send()
            .await
            .unwrap();
        assert_eq!(wrong.status().as_u16(), 400);
        let success = browser.get(&success).send().await.unwrap();
        assert_eq!(success.status().as_u16(), 200);
        let body = success.text().await.unwrap();
        assert!(body.contains("授权回调已接收"));
        assert!(!body.contains("synthetic-auth-code"));
    };
    TEST_ORIGIN
        .scope(peer.origin.clone(), async {
            let (received, ()) = tokio::join!(login.complete(), browser);
            let session = received.unwrap().admit().unwrap();
            let catalog = session.model_catalog().await.unwrap();
            assert_eq!(catalog[0].model_id, "live-model");
            let mut refreshed = session
                .into_refresh_grant("synthetic-access", 401)
                .unwrap()
                .execute()
                .await
                .unwrap();
            let successor = refreshed.admit().unwrap();
            assert_eq!(successor.access_token(), "synthetic-next");
            assert_eq!(successor.refresh_token(), "synthetic-refresh");
        })
        .await;
    peer.inspect(|seen| {
        assert_eq!(seen.len(), 3);
        assert_eq!(seen[0].method, "POST");
        assert_eq!(seen[0].path, "/api/auth/exchange");
        assert_eq!(seen[0].body["authCode"], "synthetic-auth-code");
        assert_eq!(seen[0].body["firstKeyfrom"], "neuro-gateway");
        assert_eq!(seen[0].body["latestKeyfrom"], "neuro-gateway");
        assert!(seen[0].body["version"]
            .as_str()
            .unwrap()
            .starts_with("neuro-gateway/"));
        assert!(uuid::Uuid::parse_str(seen[0].body["uuid"].as_str().unwrap()).is_ok());
        assert!(seen[0].headers.get("authorization").is_none());
        assert_eq!(seen[1].method, "GET");
        assert_eq!(seen[1].path, "/api/models/available");
        assert_eq!(seen[1].headers["authorization"], "Bearer synthetic-access");
        assert_eq!(
            seen[1].headers["x-lobsterai-client-version"],
            super::runtime::VERSION
        );
        assert_eq!(
            seen[1].headers["x-lobsterai-client-capabilities"],
            super::virtual_model::CAPABILITY
        );
        assert!(seen[0]
            .headers
            .get("x-lobsterai-client-capabilities")
            .is_none());
        assert!(seen[2]
            .headers
            .get("x-lobsterai-client-capabilities")
            .is_none());
        assert!(seen[0].headers.get("x-lobsterai-client-version").is_none());
        assert!(seen[2].headers.get("x-lobsterai-client-version").is_none());
        let query = seen[1].query.as_deref().unwrap();
        assert!(query.contains("firstKeyfrom=neuro-gateway"));
        assert!(query.contains("userId=synthetic-user"));
        assert!(!query.contains("synthetic-access"));
        assert_eq!(seen[2].path, "/api/auth/refresh");
        assert_eq!(seen[2].body["refreshToken"], "synthetic-refresh");
        assert_eq!(seen[2].body["userId"], "synthetic-user");
        assert_eq!(seen[2].body["uuid"], seen[0].body["uuid"]);
        assert!(seen[2].headers.get("authorization").is_none());
        for row in seen {
            for header in ["cookie", "x-api-key", "x-account-mode"] {
                assert!(row.headers.get(header).is_none(), "{header}");
            }
        }
    });
    let addr = url::Url::parse(&redirect)
        .unwrap()
        .socket_addrs(|| None)
        .unwrap()[0];
    assert!(tokio::net::TcpListener::bind(addr).await.is_ok());
}

async fn exchange(peer: &Peer) -> Result<ReceivedSession, LoginError> {
    let auth = Authorization::new(peer.client(), "http://127.0.0.1:12345/auth/callback").unwrap();
    let callback = callback(&auth.authorization_url(), "one-shot-code");
    TEST_ORIGIN
        .scope(
            peer.origin.clone(),
            auth.into_code_grant(&callback).unwrap().exchange(),
        )
        .await
}

#[tokio::test]
async fn redirects_rejections_business_errors_and_bad_json_never_retry() {
    for (reply, expected) in [
        (
            Reply::raw(302, vec![]).header("location", "http://must-not-follow.invalid"),
            LoginError::RedirectRejected,
        ),
        (
            Reply::raw(401, b"private secret error".to_vec()),
            LoginError::ReauthorizationRequired,
        ),
        (
            Reply::raw(403, b"private secret error".to_vec()),
            LoginError::Rejected,
        ),
        (Reply::raw(429, vec![]), LoginError::Rejected),
        (Reply::raw(500, vec![]), LoginError::Rejected),
        (
            Reply::json(json!({"code":40100,"message":"private secret error"})),
            LoginError::ReauthorizationRequired,
        ),
        (
            Reply::json(json!({"code":40101})),
            LoginError::ReauthorizationRequired,
        ),
        (Reply::json(json!({"code":40300})), LoginError::Rejected),
        (
            Reply::json(json!({"code":"0","data":{}})),
            LoginError::InvalidResponse,
        ),
        (
            Reply::json(json!({"code":0,"data":[]})),
            LoginError::InvalidResponse,
        ),
        (
            Reply::raw(
                200,
                br#"{"code":0,"data":{"accessToken":"a","accessToken":"b"}}"#.to_vec(),
            ),
            LoginError::InvalidResponse,
        ),
        (
            Reply::raw(200, b"not-json-private-secret".to_vec()),
            LoginError::InvalidResponse,
        ),
        (
            Reply::raw(200, vec![0xff, 0xfe]),
            LoginError::InvalidResponse,
        ),
    ] {
        let peer = Peer::start(vec![reply]).await;
        let error = exchange(&peer).await.unwrap_err();
        assert_eq!(error, expected);
        assert!(!format!("{error:?} {error}").contains("private"));
        peer.inspect(|seen| assert_eq!(seen.len(), 1));
    }
}

#[tokio::test]
async fn received_identity_rejection_does_not_repeat_consumed_exchange() {
    let mut value = tokens();
    value["data"]["accountMode"] = "enterprise".into();
    let peer = Peer::start(vec![Reply::json(value)]).await;
    let mut received = exchange(&peer).await.unwrap();
    assert_eq!(
        received.admit().unwrap_err(),
        LoginError::EnterpriseUnsupported
    );
    assert_eq!(
        received.admit().unwrap_err(),
        LoginError::EnterpriseUnsupported
    );
    peer.inspect(|seen| assert_eq!(seen.len(), 1));
}

#[tokio::test]
async fn decoded_response_limit_and_slow_body_are_bounded() {
    let peer = Peer::start(vec![Reply::chunks(vec![
        vec![b' '; 32768],
        vec![b' '; 32769],
    ])])
    .await;
    assert_eq!(
        exchange(&peer).await.unwrap_err(),
        LoginError::ResponseTooLarge
    );
    peer.inspect(|seen| assert_eq!(seen.len(), 1));
    let peer = Peer::start(vec![Reply::stalled()]).await;
    let error = TEST_BUDGET
        .scope(Duration::from_millis(100), exchange(&peer))
        .await
        .unwrap_err();
    assert!(matches!(error, LoginError::Timeout | LoginError::Transport));
    peer.inspect(|seen| assert_eq!(seen.len(), 1));
}

#[tokio::test]
async fn refresh_wire_keeps_successor_receipt_without_publishing_wrong_identity() {
    let peer = Peer::start(vec![Reply::json(json!({"code":0,"data":{
        "accessToken":"next-a", "refreshToken":"next-r", "user":{"yid":"wrong-user"}}}))])
    .await;
    let grant = peer
        .receipt()
        .admit()
        .unwrap()
        .into_refresh_grant("synthetic-access", 401)
        .unwrap();
    let mut received = TEST_ORIGIN
        .scope(peer.origin.clone(), grant.execute())
        .await
        .unwrap();
    assert_eq!(received.admit().unwrap_err(), LoginError::IdentityMismatch);
    peer.inspect(|seen| assert_eq!(seen.len(), 1));
}

#[tokio::test]
async fn catalog_401_is_reported_without_implicit_refresh() {
    for status in [401, 403] {
        let peer = Peer::start(vec![Reply::raw(status, b"secret".to_vec())]).await;
        let session = peer.receipt().admit().unwrap();
        let error = TEST_ORIGIN
            .scope(peer.origin.clone(), session.model_catalog())
            .await
            .unwrap_err();
        assert_eq!(
            error,
            if status == 401 {
                LoginError::ReauthorizationRequired
            } else {
                LoginError::Rejected
            }
        );
        peer.inspect(|seen| {
            assert_eq!(seen.len(), 1);
            assert_eq!(seen[0].path, "/api/models/available");
        });
    }
}

#[tokio::test]
async fn owned_transport_overrides_a_callers_post_retry_policy() {
    fn retry_policy() -> rquest::retry::Policy {
        rquest::retry::Policy::for_host("127.0.0.1")
            .no_budget()
            .max_retries_per_request(1)
            .classify_fn(|result| {
                if result.status().is_some_and(|s| s.as_u16() == 500) {
                    result.retryable()
                } else {
                    result.success()
                }
            })
    }
    // Positive control proves this builder would really replay a POST without our override.
    let control = Peer::start(vec![Reply::raw(500, vec![]), Reply::json(tokens())]).await;
    let http = control.builder().retry(retry_policy()).build().unwrap();
    let response = http
        .post(format!("{}/api/auth/exchange", control.origin))
        .json(&json!({"authCode":"synthetic-control"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    control.inspect(|seen| assert_eq!(seen.len(), 2));
    for refresh in [false, true] {
        let peer = Peer::start(vec![Reply::raw(500, vec![]), Reply::json(tokens())]).await;
        let client =
            Client::new(uuid::Uuid::new_v4(), peer.builder().retry(retry_policy())).unwrap();
        let error = TEST_ORIGIN
            .scope(peer.origin.clone(), async {
                if refresh {
                    let session = ReceivedSession::capture(client, tokens(), None)
                        .unwrap()
                        .admit()
                        .unwrap();
                    session
                        .into_refresh_grant("synthetic-access", 401)
                        .unwrap()
                        .execute()
                        .await
                        .unwrap_err()
                } else {
                    let auth =
                        Authorization::new(client, "http://127.0.0.1:12345/auth/callback").unwrap();
                    let callback = callback(&auth.authorization_url(), "synthetic-code");
                    auth.into_code_grant(&callback)
                        .unwrap()
                        .exchange()
                        .await
                        .unwrap_err()
                }
            })
            .await;
        assert_eq!(error, LoginError::Rejected);
        peer.inspect(|seen| assert_eq!(seen.len(), 1));
    }
}

#[tokio::test]
async fn cancelling_pending_login_releases_listening_and_active_sockets() {
    let login = PendingLogin::bind(client()).await.unwrap();
    let address = url::Url::parse(login.redirect_uri())
        .unwrap()
        .socket_addrs(|| None)
        .unwrap()[0];
    let handle = tokio::spawn(login.receive());
    let mut stream = TcpStream::connect(address).await.unwrap();
    stream
        .write_all(b"GET /auth/callback HTTP/1.1\r\n")
        .await
        .unwrap();
    handle.abort();
    assert!(handle.await.unwrap_err().is_cancelled());
    assert!(tokio::net::TcpListener::bind(address).await.is_ok());
    let mut bytes = [0; 1];
    let closed = tokio::time::timeout(Duration::from_secs(1), stream.read(&mut bytes))
        .await
        .unwrap();
    assert!(closed.is_err() || closed.unwrap() == 0);
}

#[tokio::test]
async fn timeout_closes_pending_login_without_any_exchange() {
    let mut login = PendingLogin::bind(client()).await.unwrap();
    let address = url::Url::parse(login.redirect_uri())
        .unwrap()
        .socket_addrs(|| None)
        .unwrap()[0];
    login.expire_in(Duration::from_millis(20));
    assert_eq!(login.receive().await.unwrap_err(), LoginError::Expired);
    assert!(tokio::net::TcpListener::bind(address).await.is_ok());
}

#[test]
fn callback_parser_rejects_wrong_host_smuggling_and_non_get() {
    let redirect = "http://127.0.0.1:12345/auth/callback";
    let valid = "GET /auth/callback?code=c&state=s HTTP/1.1\r\nHost: 127.0.0.1:12345\r\n\r\n";
    let parse =
        |head: &str| crate::protocol::loopback_callback::callback(head.as_bytes(), redirect);
    assert!(parse(valid).is_some());
    for invalid in [
        valid.replace("GET", "POST"),
        valid.replace("127.0.0.1:12345", "evil.invalid"),
        valid.replace("/auth/callback?", "http://evil.invalid/auth/callback?"),
        valid.replace("\r\n\r\n", "\r\nHost: 127.0.0.1:12345\r\n\r\n"),
        valid.replace("\r\n\r\n", "\r\nTransfer-Encoding: chunked\r\n\r\n"),
        valid.replace("\r\n\r\n", "\r\nContent-Length: 1\r\n\r\n"),
        format!("{valid}GET / HTTP/1.1\r\n\r\n"),
    ] {
        assert!(parse(&invalid).is_none());
    }
}
