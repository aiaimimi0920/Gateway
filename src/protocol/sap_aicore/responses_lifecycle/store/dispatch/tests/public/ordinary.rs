//! Native non-SAP store=true keeps the ordinary pipeline and caller body.
use super::*;

#[tokio::test]
async fn sap_aicore_stored_http_does_not_capture_ordinary_native_responses() {
    Box::pin(async {
        let (_, fixture, unused) = prepare(Arc::new(Wire::default())).await;
        unused.abort();
        let _ = unused.await;
        let calls = Arc::new(Mutex::new(Vec::<Value>::new()));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let app = Router::new().route(
            "/v1/responses",
            axum::routing::post({
                let calls = calls.clone();
                move |Json(body): Json<Value>| {
                    let calls = calls.clone();
                    async move {
                        calls.lock().unwrap().push(body);
                        Json(
                            json!({"id":"resp_ordinary","object":"response","status":"completed",
                        "store":true,"model":"ordinary-upstream",
                        "output":[{"id":"msg_1","type":"message","role":"assistant",
                            "content":[{"type":"output_text","text":"ordinary","annotations":[]}]}],
                        "usage":{"input_tokens":2,"output_tokens":3,"total_tokens":5}}),
                        )
                    }
                }
            }),
        );
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let document = json!({"providers":[{"id":"ordinary","preset":"openai","base_url":origin,
            "explicit_credentials_only":true,"supported_models":[MODEL],
            "model_map":{MODEL:"ordinary-upstream"},
            "credentials":[{"id":"ordinary-key","api_key":"synthetic"}]}],"model_routes":[]});
        fixture
            .state
            .route_config
            .replace_document(serde_json::from_value(document).unwrap())
            .unwrap();
        let mut body = create();
        body["previous_response_id"] = "resp_native_predecessor".into();
        body["metadata"] = json!({"opaque":"unchanged"});
        let (status, response) = read(fixture.open("responses", body).await).await;
        assert_eq!(status, StatusCode::OK, "{response}");
        {
            let calls = calls.lock().unwrap();
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0]["store"], true);
            assert_eq!(calls[0]["previous_response_id"], "resp_native_predecessor");
            assert_eq!(calls[0]["metadata"]["opaque"], "unchanged");
        }
        assert_eq!(fixture.remaining().await, 1);
        assert_eq!(
            fixture.audits(1).await[0].provider_account_id.as_deref(),
            Some("ordinary")
        );
        // A Gateway receipt reference must never leak to an ordinary native upstream.
        let mut body = create();
        body["previous_response_id"] = format!("resp_gw_{}", "a".repeat(32)).into();
        assert_eq!(
            read(fixture.open("responses", body).await).await.0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(calls.lock().unwrap().len(), 1);
        assert_eq!(fixture.remaining().await, 1);
        fixture.finish().await;
        server.abort();
        let _ = server.await;
    })
    .await;
}
