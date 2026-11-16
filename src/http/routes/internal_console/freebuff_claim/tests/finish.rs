use super::*;
#[tokio::test]
async fn freebuff_claim_management_ends_original_removed_model_with_exact_signed_delete_and_history(
) {
    use base64::Engine;
    for refund in [0.0, 4.25] {
        let (root, f) = fixture().await;
        let grant = grant_console_secret_access(&f.state).await;
        let peer = wire::Peer::session(
            json!({"status":"ended","freebucksRefund":refund,"upstreamSecret":"never-publish"}),
            200,
            None,
        )
        .await;
        let (id, public) = seed(&f, &peer, &grant).await;
        let mut document = f.state.route_config.snapshot().document().clone();
        document.providers[0].credentials[1]
            .supported_models
            .clear();
        f.state.route_config.replace_document(document).unwrap();
        let response = call(
            &f,
            &peer,
            &format!("{ENDPOINT}/end"),
            end_body(&f, &id),
            &grant,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["cache-control"], "no-store");
        let value = parse_json(response).await;
        assert_eq!(value["localReleased"], true);
        assert_eq!(value["fromHistory"], false);
        assert_eq!(value["receipt"]["refund"], refund);
        assert_eq!(value["modelKey"], "open");
        assert_eq!(counts(&f).await, (0, 1));
        for secret in [
            "synthetic-b",
            "upstreamSecret",
            "never-publish",
            "device-fixture",
            "sealed",
            "fetchId",
            "fbm1.",
        ] {
            assert!(!value.to_string().contains(secret), "{secret}");
        }
        {
            let hits = peer.hits.lock().unwrap();
            assert_eq!(hits.len(), 2);
            assert_eq!(hits[0].0, "GET");
            assert_eq!(hits[0].1, "/api/v1/freebuff/models");
            let (method, path, headers, bytes) = &hits[1];
            assert_eq!(method, "DELETE");
            assert_eq!(path, "/api/v1/freebuff/session/attempt");
            assert!(bytes.is_empty());
            assert_eq!(headers["authorization"], "Bearer synthetic-b");
            assert_eq!(headers["x-freebuff-instance-id"], format!("cli:{id}"));
            assert_eq!(headers["x-freebuff-desktop-attempt-id"], id);
            let signature = base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(headers["x-freebuff-device-sig"].as_bytes())
                .unwrap();
            let public = base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(public)
                .unwrap();
            let hash = hex::encode(ring::digest::digest(&ring::digest::SHA256, b""));
            let signed = format!(
                "freebuff-device-v1\nDELETE\n{path}\n{}\n{hash}\nsynthetic-fetch",
                headers["x-freebuff-device-ts"].to_str().unwrap()
            );
            ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, public)
                .verify(signed.as_bytes(), &signature)
                .unwrap();
        }
        peer.hits.lock().unwrap().clear();
        let response = call(
            &f,
            &peer,
            &format!("{ENDPOINT}/end"),
            end_body(&f, &id),
            &grant,
        )
        .await;
        assert_eq!(parse_json(response).await["fromHistory"], true);
        let response = call(&f, &peer, ENDPOINT, body(&f), &grant).await;
        let journal = parse_json(response).await;
        assert!(journal["claim"].is_null());
        assert_eq!(journal["history"][0]["attemptId"], id);
        assert!(peer.hits.lock().unwrap().is_empty());
        peer.close().await;
        cleanup(&root, f).await;
    }
}
#[tokio::test]
async fn freebuff_claim_management_nonfinal_or_failed_end_keeps_original_obligation() {
    for (body, status, ok) in [
        (json!({"status":"ended"}), 200, true),
        (
            json!({"status":"ended","freebucksRefund":0,"freebucksRefundPending":true}),
            200,
            true,
        ),
        (json!({"status":"ended","freebucksRefund":0}), 500, false),
        (json!({"status":"ended","freebucksRefund":0}), 307, false),
    ] {
        let (root, f) = fixture().await;
        let grant = grant_console_secret_access(&f.state).await;
        let peer = wire::Peer::session(body, status, None).await;
        let (id, _) = seed(&f, &peer, &grant).await;
        let response = call(
            &f,
            &peer,
            &format!("{ENDPOINT}/end"),
            end_body(&f, &id),
            &grant,
        )
        .await;
        assert_eq!(response.status().is_success(), ok);
        if ok {
            assert_eq!(parse_json(response).await["localReleased"], false);
        }
        assert_eq!(counts(&f).await, (1, 0));
        assert_eq!(peer.hits.lock().unwrap().len(), 2);
        peer.close().await;
        cleanup(&root, f).await;
    }
}
#[tokio::test]
async fn freebuff_claim_management_real_grant_or_revision_revocation_after_delete_retains_record() {
    for revoke in [true, false] {
        let (root, f) = fixture().await;
        let grant = grant_console_secret_access(&f.state).await;
        let state = f.state.clone();
        let peer = wire::Peer::session(
            json!({"status":"ended","freebucksRefund":0}),
            200,
            Some(Arc::new(move || {
                if revoke {
                    let request =
                        super::super::super::console_request_context(&HeaderMap::new(), None);
                    state
                        .console_auth
                        .logout(&request, support::MANAGEMENT_TOKEN)
                        .unwrap();
                } else {
                    let mut document = state.route_config.snapshot().document().clone();
                    document.providers[0].credentials[0].account_name = Some("changed".into());
                    state.route_config.replace_document(document).unwrap();
                }
            })),
        )
        .await;
        let (id, _) = seed(&f, &peer, &grant).await;
        let response = call(
            &f,
            &peer,
            &format!("{ENDPOINT}/end"),
            end_body(&f, &id),
            &grant,
        )
        .await;
        assert_eq!(
            response.status(),
            if revoke {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::CONFLICT
            }
        );
        assert_eq!(counts(&f).await, (1, 0));
        assert_eq!(peer.hits.lock().unwrap().len(), 2);
        peer.close().await;
        cleanup(&root, f).await;
    }
}
