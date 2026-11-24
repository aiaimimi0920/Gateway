//! Loopback XSUAA requires a verified client certificate; inference uses a separate listener.
use super::super::test_support;
use super::*;
use axum::{body::Body, http::HeaderMap, response::Response};
use hyper::{body::Incoming, server::conn::http1, service::service_fn};
use hyper_util::rt::TokioIo;
use rustls::pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer};
use std::{convert::Infallible, sync::Mutex};
use tokio::{net::TcpListener, task::JoinSet};
use tokio_rustls::TlsAcceptor;

pub(super) fn root_pem() -> &'static [u8] {
    &mtls_fixtures::get().root_pem
}

#[derive(Default)]
struct Exchange {
    calls: Vec<(String, HeaderMap, String)>,
    redirect: bool,
}
pub(super) struct OAuthPeer {
    pub origin: String,
    exchange: Arc<Mutex<Exchange>>,
    task: tokio::task::JoinHandle<()>,
}
impl OAuthPeer {
    pub(super) async fn start() -> Self {
        let material = mtls_fixtures::get();
        let mut roots = rustls::RootCertStore::empty();
        roots
            .add(CertificateDer::from(material.root_der.as_slice()))
            .unwrap();
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let verifier = rustls::server::WebPkiClientVerifier::builder_with_provider(
            Arc::new(roots),
            provider.clone(),
        )
        .build()
        .unwrap();
        let config = rustls::ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_client_cert_verifier(verifier)
            .with_single_cert(
                vec![CertificateDer::from(material.server_der.as_slice()).into_owned()],
                PrivateKeyDer::from_pem_slice(&material.server_key).unwrap(),
            )
            .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!(
            "https://localhost:{}",
            listener.local_addr().unwrap().port()
        );
        let exchange = Arc::new(Mutex::new(Exchange::default()));
        let state = exchange.clone();
        let task = tokio::spawn(async move {
            let acceptor = TlsAcceptor::from(Arc::new(config));
            let mut connections = JoinSet::new();
            loop {
                tokio::select! {
                    next = listener.accept() => {
                        let (stream, _) = next.unwrap();
                        let acceptor = acceptor.clone();
                        let state = state.clone();
                        connections.spawn(async move {
                            let Ok(Ok(tls)) = tokio::time::timeout(Duration::from_secs(3), acceptor.accept(stream)).await else { return };
                            let app = service_fn(move |request| handle(state.clone(), request));
                            let _ = http1::Builder::new().serve_connection(TokioIo::new(tls), app).await;
                        });
                    }
                    _ = connections.join_next(), if !connections.is_empty() => {}
                }
            }
        });
        Self {
            origin,
            exchange,
            task,
        }
    }
    pub(super) fn assert_calls(&self, expected: &[&str]) {
        let exchange = self.exchange.lock().unwrap();
        assert_eq!(exchange.calls.len(), expected.len());
        for ((path, headers, body), client_id) in exchange.calls.iter().zip(expected) {
            assert_eq!(path, "/oauth/token");
            assert_eq!(headers["content-type"], "application/x-www-form-urlencoded");
            for header in ["authorization", "cookie", "AI-Resource-Group"] {
                assert!(!headers.contains_key(header), "{header}");
            }
            assert_eq!(
                body,
                &format!("grant_type=client_credentials&client_id={client_id}")
            );
        }
    }
}
impl Drop for OAuthPeer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn handle(
    state: Arc<Mutex<Exchange>>,
    request: Request<Incoming>,
) -> Result<Response, Infallible> {
    let (parts, body) = request.into_parts();
    let form = drain(Body::new(body)).await;
    let mut exchange = state.lock().unwrap();
    assert_eq!(parts.method, "POST");
    exchange
        .calls
        .push((parts.uri.path().into(), parts.headers, form));
    if exchange.redirect {
        return Ok(Response::builder()
            .status(307)
            .header("location", "/unexpected")
            .body(Body::empty())
            .unwrap());
    }
    Ok(Response::builder()
        .status(200)
        .body(Body::from(
            json!({
                "access_token":"synthetic-sap-access", "token_type":"Bearer", "expires_in":3600
            })
            .to_string(),
        ))
        .unwrap())
}

#[tokio::test]
async fn sap_aicore_x509_public_inference_reuses_token_and_settles() {
    Box::pin(async {
        let oauth = OAuthPeer::start().await;
        let peer = peer::Peer::new("chat", false, false, 200).await;
        let fixture = Box::pin(Fixture::with_key(
            &peer,
            &mtls_fixtures::service_key(false).to_string(),
        ))
        .await;
        test_support::origin(
            peer.origin.clone(),
            test_support::mtls(oauth.origin.clone(), root_pem(), async {
                for stream in [false, true] {
                    let response = fixture
                        .open("chat/completions", body("chat/completions", stream))
                        .await;
                    let status = response.status();
                    let text = drain(response.into_body()).await;
                    assert_eq!(status, StatusCode::OK, "{text}");
                    assert!(text.contains("合成答案"));
                    assert_eq!(text.contains("[DONE]"), stream);
                }
                for row in fixture.audits(2).await {
                    assert_identity(&row);
                    assert_eq!(row.total_tokens, Some(5));
                    assert_eq!(row.status, "completed");
                    assert!(!serde_json::to_string(&row).unwrap().contains("PRIVATE KEY"));
                }
                assert_eq!(fixture.remaining().await, 0);
            }),
        )
        .await;
        oauth.assert_calls(&["synthetic-sap-client"]);
        assert!(peer.oauth.lock().unwrap().is_empty());
        {
            let calls = peer.calls.lock().unwrap();
            assert_eq!(calls.len(), 2);
            for (_, headers, raw) in calls.iter() {
                assert_eq!(headers["authorization"], "Bearer synthetic-sap-access");
                assert_eq!(headers["AI-Resource-Group"], "group-a");
                assert!(!raw.to_string().contains("PRIVATE KEY"));
            }
        }
        fixture.finish().await;
        peer.stop().await;
    })
    .await;
}

#[tokio::test]
async fn sap_aicore_x509_invalid_tls_and_redirect_do_not_generate_or_spend() {
    Box::pin(async {
        let oauth = OAuthPeer::start().await;
        oauth.exchange.lock().unwrap().redirect = true;
        let peer = peer::Peer::new("chat", false, false, 200).await;
        let fixture = Box::pin(Fixture::with_key(
            &peer,
            &mtls_fixtures::service_key(false).to_string(),
        ))
        .await;
        for (target, expected_status, expected_code) in [
            (
                oauth.origin.replace("localhost", "127.0.0.1"),
                StatusCode::BAD_GATEWAY,
                "sap_aicore_token_failed",
            ),
            (
                oauth.origin.clone(),
                StatusCode::TEMPORARY_REDIRECT,
                "sap_aicore_token_rejected",
            ),
        ] {
            test_support::origin(
                peer.origin.clone(),
                test_support::mtls(target, root_pem(), async {
                    let response = fixture
                        .open("chat/completions", body("chat/completions", false))
                        .await;
                    let status = response.status();
                    assert!(
                        response.headers().get("location").is_none(),
                        "never forward OAuth redirect targets"
                    );
                    let text = drain(response.into_body()).await;
                    assert_eq!(status, expected_status, "{text}");
                    assert!(text.contains(expected_code), "{text}");
                    assert!(!text.contains("PRIVATE KEY") && !text.contains("localhost"));
                }),
            )
            .await;
        }
        oauth.assert_calls(&["synthetic-sap-client"]);
        assert!(peer.oauth.lock().unwrap().is_empty());
        assert!(peer.calls.lock().unwrap().is_empty());
        assert!(fixture
            .audits(2)
            .await
            .iter()
            .all(|row| row.status != "completed"));
        assert_eq!(fixture.remaining().await, 2);
        fixture.finish().await;
        peer.stop().await;
    })
    .await;
}

#[tokio::test]
async fn sap_aicore_x509_rotated_certificate_does_not_borrow_cached_token() {
    let oauth = OAuthPeer::start().await;
    let cache = super::super::auth::TokenCache::default();
    let mut payload = super::super::tests::payload();
    let http = rquest::Client::new();
    test_support::mtls(oauth.origin.clone(), root_pem(), async {
        payload.api_key = mtls_fixtures::service_key(false).to_string();
        assert!(cache.get(&http, &payload).await.is_ok());
        assert!(cache.get(&http, &payload).await.is_ok());
        oauth.assert_calls(&["synthetic-sap-client"]);
        payload.api_key = mtls_fixtures::service_key(true).to_string();
        assert!(cache.get(&http, &payload).await.is_ok());
        oauth.assert_calls(&["synthetic-sap-client", "synthetic-sap-client"]);
    })
    .await;
}
