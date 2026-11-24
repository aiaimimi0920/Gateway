//! Exercise stored SSE through the public router, loopback OAuth and inference.
use super::*;
use http_body_util::BodyExt;

#[tokio::test]
async fn sap_aicore_stored_background_sse_resume() {
    Box::pin(async {
        let release = Arc::new(tokio::sync::Notify::new());
        let remote = "resp_stored_remote";
        let wire = Arc::new(Wire {
            stream_response: Some((
                event("response.created", 0, snapshot("queued", remote)),
                event("response.queued", 1, snapshot("queued", remote)),
                release.clone(),
            )),
            resume_response: Some(event(
                "response.completed",
                2,
                snapshot("completed", remote),
            )),
            ..Wire::default()
        });
        let (origin, fixture, server) = prepare(wire.clone()).await;
        test_support::origin(origin, async {
            let mut req = create();
            req["stream"] = true.into();
            req["background"] = true.into();
            let response = fixture.open("responses", req).await;
            assert_eq!(response.status(), StatusCode::OK);
            let mut body = response.into_body();
            let created = receive(&mut body).await;
            let local = created["response"]["id"].as_str().unwrap();
            assert_eq!(created["response"]["status"], "queued");
            release.notify_one();
            assert_eq!(receive(&mut body).await["sequence_number"], 1);
            // The first stream ended without a terminal frame; recover by GET only.
            let _ = body.collect().await;
            let router = crate::http::router::build_router(fixture.state.clone());
            let get = |query: &str| {
                Request::builder()
                    .uri(format!("/v1/responses/{local}?{query}"))
                    .header("authorization", format!("Bearer {}", fixture.token))
                    .body(Body::empty())
                    .unwrap()
            };
            let rejected = router
                .clone()
                .oneshot(get("starting_after=1"))
                .await
                .unwrap();
            assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
            for _ in 0..2 {
                let response = router
                    .clone()
                    .oneshot(get("stream=true&starting_after=1"))
                    .await
                    .unwrap();
                assert_eq!(response.status(), StatusCode::OK);
                assert_eq!(response.headers()["content-type"], "text/event-stream");
                let mut body = response.into_body();
                let done = receive(&mut body).await;
                assert_eq!(done["sequence_number"], 2);
                assert_eq!(done["response"]["id"], local);
                assert_eq!(done["response"]["usage"]["total_tokens"], 5);
            }
            assert_eq!(wire.creates.lock().unwrap().len(), 1);
            assert_eq!(wire.creates.lock().unwrap()[0]["background"], true);
            let pool = &fixture.state.local_runtime.as_ref().unwrap().pool;
            let result: (String, i64, i64) = sqlx::query_as(
                "SELECT r.phase,r.settled,h.open FROM gateway_sap_response_receipts r
                 JOIN gateway_sap_response_balance_holds h ON h.response_id=r.id WHERE r.id=$1",
            )
            .bind(local)
            .fetch_one(pool)
            .await
            .unwrap();
            assert_eq!(result, ("completed".into(), 1, 0));
        })
        .await;
        server.abort();
    })
    .await;
}

fn event(name: &str, sequence: u64, mut fields: Value) -> String {
    fields["type"] = name.into();
    fields["sequence_number"] = sequence.into();
    format!("event: {name}\ndata: {fields}\n\n")
}

fn snapshot(status: &str, id: &str) -> Value {
    let mut response = json!({"id":id,"object":"response","status":status,"store":true,
        "model":"sap-private-name","output":[],"metadata":{"private":"never-publish"}});
    if status == "completed" {
        response["usage"] = json!({"input_tokens":2,"output_tokens":3,"total_tokens":5});
    }
    json!({"response":response})
}

async fn receive(body: &mut Body) -> Value {
    let frame = tokio::time::timeout(std::time::Duration::from_secs(5), body.frame())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
        .into_data()
        .unwrap();
    let text = std::str::from_utf8(&frame).unwrap();
    assert!(!text.contains("sap-private-name") && !text.contains("never-publish"));
    assert!(!text.contains("resp_stored_remote"));
    serde_json::from_str(
        text.lines()
            .find_map(|line| line.strip_prefix("data: "))
            .unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn sap_aicore_stored_sse_public_live_and_recoverable_errors() {
    for mode in ["complete", "bad_tail", "changed_id", "disconnect"] {
        Box::pin(exercise(mode)).await;
    }
}

#[tokio::test]
async fn sap_aicore_stored_sse_public_slow_subscriber_and_concurrent_get() {
    for mode in [
        "overflow",
        "get_matches",
        "get_conflicts",
        "get_then_delete",
    ] {
        Box::pin(exercise(mode)).await;
    }
}

async fn exercise(mode: &str) {
    let release = Arc::new(tokio::sync::Notify::new());
    let id = "resp_stored_remote";
    let delta_count = if mode == "overflow" { 24 } else { 1 };
    let deltas = (1..=delta_count)
        .map(|sequence| {
            event(
                "response.output_text.delta",
                sequence,
                json!({
                    "item_id":"msg_1","output_index":0,"content_index":0,"delta":"hello",
                    "extra":"never-publish"
                }),
            )
        })
        .collect::<String>();
    let done = event(
        "response.completed",
        delta_count + 1,
        snapshot(
            "completed",
            if mode == "changed_id" {
                "resp_other"
            } else {
                id
            },
        ),
    );
    let extra = if mode == "bad_tail" {
        "data: {broken}\n\n"
    } else {
        ""
    };
    let wire = Arc::new(Wire {
        stream_response: Some((
            event("response.created", 0, snapshot("in_progress", id)),
            format!("{deltas}{done}{extra}"),
            release.clone(),
        )),
        terminal_response: (mode == "get_conflicts").then(|| {
            let mut response = snapshot("completed", id)["response"].clone();
            response["usage"] = json!({"input_tokens":7,"output_tokens":3,"total_tokens":10});
            response
        }),
        ..Wire::default()
    });
    let (origin, fixture, server) = prepare(wire.clone()).await;
    test_support::origin(origin, async {
        let mut req = create();
        req["stream"] = true.into();
        let response = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            fixture.open("responses", req),
        )
        .await
        .unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{mode}");
        assert_eq!(response.headers()["content-type"], "text/event-stream");
        let mut body = response.into_body();
        let created = receive(&mut body).await;
        assert_eq!(created["type"], "response.created");
        assert_eq!(created["response"]["model"], MODEL);
        let local = created["response"]["id"].as_str().unwrap();
        assert!(local.starts_with("resp_gw_"));
        let pool = &fixture.state.local_runtime.as_ref().unwrap().pool;
        let read =
            || {
                sqlx::query_as::<_, (String, String, i64, i64)>(
            "SELECT r.upstream_id,r.phase,r.settled,h.open FROM gateway_sap_response_receipts r
             JOIN gateway_sap_response_balance_holds h ON h.response_id=r.id WHERE r.id=$1")
            .bind(local).fetch_one(pool)
            };
        assert_eq!(read().await.unwrap(), (id.into(), "pending".into(), 0, 1));
        let path = format!("/v1/responses/{local}");
        let concurrent_get = mode.starts_with("get_");
        if concurrent_get {
            let (status, loaded) = at(&fixture, "GET", &path).await;
            assert_eq!(status, StatusCode::OK, "{loaded}");
            assert_eq!(
                loaded["usage"]["total_tokens"],
                if mode == "get_conflicts" { 10 } else { 5 }
            );
            if mode == "get_then_delete" {
                let (status, deleted) = at(&fixture, "DELETE", &path).await;
                assert_eq!(status, StatusCode::OK, "{deleted}");
                assert_eq!(deleted["deleted"], true);
            }
        }
        let mut body = Some(body);
        if matches!(mode, "disconnect" | "overflow") {
            if mode == "disconnect" {
                drop(body.take());
                fixture.state.lifecycle.begin_drain("stored-sse-test");
            }
            release.notify_one();
            tokio::time::timeout(
                std::time::Duration::from_secs(5),
                fixture.state.lifecycle.stored_response_tasks.join(),
            )
            .await
            .unwrap();
            if let Some(body) = body.as_mut() {
                let mut count = 0;
                loop {
                    let frame = body.frame().await.expect("overflow must fail the stream");
                    if frame.is_err() {
                        break;
                    }
                    count += 1;
                    let frame = frame.unwrap().into_data().unwrap();
                    assert!(std::str::from_utf8(&frame)
                        .unwrap()
                        .contains("response.output_text.delta"));
                }
                assert!(count > 0 && count < delta_count);
            }
        } else {
            release.notify_one();
            let body = body.as_mut().unwrap();
            assert_eq!(receive(body).await["delta"], "hello");
            if matches!(mode, "complete" | "get_matches") {
                let terminal = receive(body).await;
                assert_eq!(terminal["type"], "response.completed");
                assert_eq!(terminal["response"]["id"], local);
                assert_eq!(terminal["response"]["usage"]["total_tokens"], 5);
                assert!(body.frame().await.is_none());
            } else {
                assert!(body.frame().await.unwrap().is_err(), "{mode}");
            }
        }
        // The public Body can retain the router's AppState even after EOF.
        drop(body);
        let needs_get = matches!(mode, "bad_tail" | "changed_id");
        let deleted = mode == "get_then_delete";
        let row = read().await.unwrap();
        if needs_get {
            assert_eq!(row, (id.into(), "pending".into(), 0, 1));
        } else {
            assert_eq!(
                row,
                (
                    id.into(),
                    if deleted { "deleted" } else { "completed" }.into(),
                    1,
                    0
                )
            );
        }
        let sent = 1 + usize::from(concurrent_get) + usize::from(deleted);
        let audits = fixture.audits(sent).await;
        let trace = audits
            .iter()
            .filter_map(|audit| audit.route_trace.as_ref())
            .find(|trace| trace["storedResponseOperation"] == "create")
            .unwrap();
        assert_eq!(trace["storedResponseId"], local);
        assert_eq!(trace["routeAttemptCount"], 1);
        assert_eq!(trace["callerDisconnected"], mode == "disconnect");
        assert_eq!(fixture.remaining().await, 1);
        assert_eq!(wire.requests.lock().unwrap().len(), sent);
        if needs_get {
            let (status, loaded) = at(&fixture, "GET", &path).await;
            assert_eq!(status, StatusCode::OK, "{loaded}");
            assert_eq!(loaded["id"], local);
            assert_eq!(read().await.unwrap(), (id.into(), "completed".into(), 1, 0));
            assert_eq!(fixture.remaining().await, 1);
            assert_eq!(wire.requests.lock().unwrap().len(), 2);
        }
    })
    .await;
    fixture.finish().await;
    server.abort();
    let _ = server.await;
}
