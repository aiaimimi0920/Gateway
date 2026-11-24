use super::super::super::State as StoredState;
use super::super::create::{submit, Target};
use super::super::{authorization, RoutePin};
use super::*;
use crate::{
    auth::{adapter::AuthRequest, authenticate_request},
    protocol::sap_aicore::{
        test_support::{self, BASE, KEY},
        wire_tests::fixture::Fixture,
    },
};
use axum::{
    body::{to_bytes, Body},
    extract::State,
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
    Json, Router,
};
use serde_json::json;
use std::sync::{Arc, Mutex};

mod public;

#[derive(Default)]
struct Wire {
    requests: Mutex<Vec<(String, String)>>,
    oauth_fail: std::sync::atomic::AtomicBool,
    oauth_calls: std::sync::atomic::AtomicUsize,
    creates: Mutex<Vec<Value>>,
    pause_create: Option<(Arc<tokio::sync::Notify>, Arc<tokio::sync::Notify>)>,
    create_response: Option<Value>,
    terminal_response: Option<Value>,
    rejection: Mutex<Option<(u16, Vec<u8>)>>,
    hold_body: bool,
}

fn scoped_loopback_http() -> rquest::Client {
    // The general wire fixture installs a rejecting proxy that answers OAuth
    // itself. Here OAuth and inference must both hit this test's scoped listener.
    rquest::Client::builder().no_proxy().build().unwrap()
}

#[tokio::test]
async fn sap_aicore_response_receipts_submit_rewrites_predecessor_and_retains_unknown_hold() {
    Box::pin(exercise_submit()).await;
}

async fn exercise_submit() {
    let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", socket.local_addr().unwrap());
    let wire = Arc::new(Wire::default());
    let app = Router::new().fallback(handler).with_state(wire.clone());
    let server = tokio::spawn(async move { axum::serve(socket, app).await.unwrap() });
    let fixture = Box::pin(Fixture::configured_with_http(
        scoped_loopback_http(),
        false,
        BASE,
        KEY,
        json!({"sap_native_responses":true,"sap_stored_responses":true,"sap_api_version":"2024-10-21"}),
    ))
    .await;
    let state = &fixture.state;
    let session = authenticate_request(
        state,
        &AuthRequest {
            authorization: Some(format!("Bearer {}", fixture.token)),
            api_key: None,
            path: "/v1/responses".into(),
            method: "POST".into(),
        },
        None,
    )
    .await
    .unwrap();
    let target = Target {
        pin: RoutePin {
            public_model: "public-sap_aicore".into(),
            source_key_ref: session.access_key_id.clone().unwrap(),
            platform_access_ref: None,
        },
        provider: "sap_aicore-public",
        credential: "exact",
        upstream_model: "deployment1",
        account_group: None,
    };
    let request = |previous: Option<&str>| {
        crate::protocol::responses::normalize_responses(json!({
            "model":"public-sap_aicore","input":"hello","store":true,"previous_response_id":previous
        }))
        .unwrap()
    };
    let pool = &state.local_runtime.as_ref().unwrap().pool;
    let store = Store::Local(pool);
    let secret = authorization::key(state).unwrap();
    let owner = authorization::owner_tag(&secret, &session).unwrap();
    let (payload, _) = authorization::read_target(
        state,
        &session,
        &target.pin,
        target.provider,
        target.credential,
        None,
    )
    .await
    .unwrap()
    .unwrap();
    let binding = target
        .pin
        .capture_binding(&secret, target.provider, target.credential, &payload)
        .unwrap();
    let unsent = store
        .begin_reserved(
            &owner,
            &binding,
            target.provider,
            target.credential,
            &target.pin,
            1,
        )
        .await
        .unwrap();
    assert_eq!(fixture.remaining().await, 1);
    store.refund_unsent(&unsent).await.unwrap();
    store.refund_unsent(&unsent).await.unwrap();
    assert_eq!(fixture.remaining().await, 2);
    assert!(!store
        .reserve_balance(&unsent, &target.pin.source_key_ref, 1)
        .await
        .unwrap());
    test_support::origin(origin, async {
        assert!(matches!(
            submit(
                state,
                &session,
                &target,
                &request(Some("resp_created")),
                1,
                SendHooks::default()
            )
            .await,
            Err(DispatchError::NotSent(_))
        ));
        assert!(wire.requests.lock().unwrap().is_empty());
        let first = submit(
            state,
            &session,
            &target,
            &request(None),
            1,
            SendHooks::default(),
        )
        .await
        .ok()
        .unwrap();
        assert_eq!(
            (first.receipt.phase.as_str(), first.receipt.settled),
            ("completed", 1)
        );
        assert!(first.receipt.id.starts_with("resp_gw_"));
        let public = first.public_json(&target.pin).unwrap();
        assert_eq!(public["id"], first.receipt.id);
        assert_eq!(public["model"], target.pin.public_model);
        assert_eq!(public["output"][0]["call_id"], "call_kept");
        for private in ["resp_created", "private-deployment", "never-publish"] {
            assert!(!public.to_string().contains(private));
        }
        let mut inconsistent = first.body.clone();
        inconsistent["usage"] = json!({"input_tokens":7,"output_tokens":3,"total_tokens":10});
        assert!(Observation {
            receipt: first.receipt,
            body: inconsistent
        }
        .public_json(&target.pin)
        .is_err());
        let second = request(public["id"].as_str());
        assert!(matches!(
            submit(state, &session, &target, &second, 1, SendHooks::default()).await,
            Err(DispatchError::Recorded(_))
        ));
        assert!(matches!(
            submit(
                state,
                &session,
                &target,
                &request(None),
                1,
                SendHooks::default()
            )
            .await,
            Err(DispatchError::NotSent(_))
        ));
        let opens: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM gateway_sap_response_balance_holds
             WHERE open = 1",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        let intents: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM gateway_sap_response_receipts r
             JOIN gateway_sap_response_balance_holds h ON h.response_id = r.id WHERE h.open = 1
             AND r.phase = 'submitting' AND r.upstream_id IS NULL",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!((opens, intents), (1, 1));
        assert_eq!(fixture.remaining().await, 0);
    })
    .await;
    let creates = std::mem::take(&mut *wire.creates.lock().unwrap());
    assert_eq!(creates.len(), 2);
    assert_eq!(creates[0]["store"], true);
    assert!(creates[0].get("previous_response_id").is_none());
    assert_eq!(creates[1]["previous_response_id"], "resp_created");
    fixture.finish().await;
    server.abort();
    let _ = server.await;
}

#[tokio::test]
async fn sap_aicore_response_receipts_dispatch_no_replay_and_no_second_debit() {
    Box::pin(exercise()).await;
}

async fn exercise() {
    let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", socket.local_addr().unwrap());
    let wire = Arc::new(Wire::default());
    let app = Router::new().fallback(handler).with_state(wire.clone());
    let server = tokio::spawn(async move { axum::serve(socket, app).await.unwrap() });
    let fixture = Box::pin(Fixture::configured_with_http(
        scoped_loopback_http(),
        false,
        BASE,
        KEY,
        json!({"sap_native_responses":true,"sap_stored_responses":true,"sap_api_version":"2024-10-21"}),
    ))
    .await;
    let state = &fixture.state;
    let session = authenticate_request(
        state,
        &AuthRequest {
            authorization: Some(format!("Bearer {}", fixture.token)),
            api_key: None,
            path: "/v1/responses".into(),
            method: "POST".into(),
        },
        None,
    )
    .await
    .unwrap();
    let key_id = session.access_key_id.as_deref().unwrap();
    let local = state.local_runtime.as_ref().unwrap();
    let store = Store::Local(&local.pool);
    let pin = RoutePin {
        public_model: "public-sap_aicore".into(),
        source_key_ref: key_id.into(),
        platform_access_ref: None,
    };
    let secret = authorization::key(state).unwrap();
    let owner = authorization::owner_tag(&secret, &session).unwrap();
    let provider = "sap_aicore-public";
    let (payload, _) = authorization::read_target(state, &session, &pin, provider, "exact", None)
        .await
        .unwrap()
        .expect("fixed fixture route");
    let binding = pin
        .capture_binding(&secret, provider, "exact", &payload)
        .unwrap();
    let mut receipts = Vec::new();
    for upstream in ["resp_first", "resp_second"] {
        let receipt = store
            .begin(&owner, &binding, provider, "exact", &pin)
            .await
            .unwrap();
        assert!(store.reserve_balance(&receipt, key_id, 1).await.unwrap());
        let pending = store
            .observe(
                &receipt,
                &UpstreamId::parse(upstream).unwrap(),
                StoredState::Pending,
            )
            .await
            .unwrap()
            .unwrap();
        receipts.push(pending);
    }
    let call = |index: usize, op| {
        access_existing(
            state,
            &session,
            &receipts[index].id,
            None,
            op,
            SendHooks::default(),
        )
    };
    test_support::origin(origin, async {
        use std::sync::atomic::Ordering::Relaxed;
        wire.oauth_fail.store(true, Relaxed);
        assert!(matches!(
            call(0, Operation::Cancel).await,
            Err(DispatchError::NotSent(_))
        ));
        assert!(wire.requests.lock().unwrap().is_empty());
        wire.oauth_fail.store(false, Relaxed);
        assert!(matches!(
            call(0, Operation::Cancel).await,
            Err(DispatchError::DeliveryUnknown)
        ));
        assert!(matches!(
            call(0, Operation::Cancel).await,
            Err(DispatchError::NotSent(_))
        ));
        let result = call(0, Operation::Retrieve).await.ok().unwrap();
        assert_eq!(
            (result.receipt.phase.as_str(), result.receipt.settled),
            ("completed", 1)
        );
        assert_eq!(result.body["id"], "resp_first"); // internal, not a public response
                                                     // Unknown cancel keeps its barrier even after a confirmed terminal GET.
        assert!(matches!(
            call(0, Operation::Delete).await,
            Err(DispatchError::NotSent(_))
        ));
        let second = call(1, Operation::Retrieve).await.ok().unwrap();
        assert_eq!(second.receipt.settled, 1);
        let deleted = call(1, Operation::Delete).await.ok().unwrap();
        assert_eq!(deleted.receipt.phase, "deleted");
        assert!(matches!(
            call(1, Operation::Retrieve).await,
            Err(DispatchError::NotSent(_))
        ));
    })
    .await;
    assert_eq!(fixture.remaining().await, 0);
    let origin_path = "/v2/inference/deployments/deployment1/responses";
    assert_eq!(
        *wire.requests.lock().unwrap(),
        [
            ("POST".into(), format!("{origin_path}/resp_first/cancel")),
            ("GET".into(), format!("{origin_path}/resp_first")),
            ("GET".into(), format!("{origin_path}/resp_second")),
            ("DELETE".into(), format!("{origin_path}/resp_second")),
        ]
    );
    fixture.finish().await;
    server.abort();
    let _ = server.await;
}

async fn handler(State(wire): State<Arc<Wire>>, request: Request<Body>) -> Response {
    let (parts, body) = request.into_parts();
    let path = parts.uri.path();
    if path == "/oauth/token" {
        wire.oauth_calls
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if wire.oauth_fail.load(std::sync::atomic::Ordering::Relaxed) {
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
        return Json(
            json!({"access_token":"stored-sap-token","token_type":"bearer","expires_in":120}),
        )
        .into_response();
    }
    wire.requests
        .lock()
        .unwrap()
        .push((parts.method.to_string(), path.to_owned()));
    assert!(parts.uri.query().is_none());
    assert_eq!(parts.headers["authorization"], "Bearer stored-sap-token");
    assert_eq!(parts.headers["ai-resource-group"], "group-a");
    if let Some((status, bytes)) = wire.rejection.lock().unwrap().as_ref() {
        return Response::builder()
            .status(*status)
            .header("retry-after", "120")
            .body(Body::from(bytes.clone()))
            .unwrap();
    }
    if wire.hold_body {
        use futures::StreamExt;
        let stream = futures::stream::once(async {
            Ok::<_, std::io::Error>(bytes::Bytes::from_static(b"{"))
        })
        .chain(futures::stream::pending());
        return Response::builder()
            .status(200)
            .body(Body::from_stream(stream))
            .unwrap();
    }
    if path.ends_with("/responses") && parts.method == axum::http::Method::POST {
        let body = serde_json::from_slice(&to_bytes(body, 2048).await.unwrap()).unwrap();
        if let Some((started, release)) = &wire.pause_create {
            started.notify_one();
            release.notified().await;
        }
        let mut creates = wire.creates.lock().unwrap();
        creates.push(body);
        if let Some(body) = &wire.create_response {
            return Json(body.clone()).into_response();
        }
        return if creates.len() == 1 {
            Json(
                json!({"id":"resp_created","model":"private-deployment","object":"response",
                "status":"completed","store":true,"previous_response_id":"never-publish",
                "output":[{"id":"fc_1","type":"function_call","call_id":"call_kept","name":"lookup",
                    "arguments":"{}","extra":"never-publish"}],
                "usage":{"input_tokens":2,"output_tokens":3,"total_tokens":5}}),
            )
            .into_response()
        } else {
            StatusCode::BAD_GATEWAY.into_response()
        };
    }
    assert!(to_bytes(body, 1).await.unwrap().is_empty());
    if path.ends_with("/cancel") {
        if let Some(body) = &wire.terminal_response {
            return Json(body.clone()).into_response();
        }
        return (
            StatusCode::TEMPORARY_REDIRECT,
            [("location", "/never-follow")],
        )
            .into_response();
    }
    let id = path.rsplit('/').next().unwrap();
    if parts.method == axum::http::Method::DELETE {
        return Json(json!({"id":id,"object":"response.deleted","deleted":true})).into_response();
    }
    if let Some(body) = &wire.terminal_response {
        return Json(body.clone()).into_response();
    }
    Json(
        json!({"id":id,"object":"response","status":"completed","store":true,"output":[],
        "usage":{"input_tokens":2,"output_tokens":3,"total_tokens":5}}),
    )
    .into_response()
}
