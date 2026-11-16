//! Additive storage compatibility, exact successor binding and unknown-lifetime rejection for legacy actors.
use super::{lobsterai_fixture::*, *};

#[tokio::test]
async fn lobsterai_result_requires_receipt_and_exact_generation_with_no_expiry() {
    let f = fixture().await;
    let request = request(&f);
    let tokens = ReceivedTokens::from_json(reply().to_string().as_bytes()).unwrap();
    let next = tokens
        .interpret_successor(
            request.material.lobsterai.as_ref().unwrap(),
            &request.material.access,
            &request.material.refresh,
        )
        .unwrap();
    let result = request.result_with_lobster(&next).unwrap();
    let record = GrantRecord::sending(&request);
    assert!(record.with_result(&result).is_err());
    let record = record
        .with_receipt(&GrantReceipt::LobsterAi { tokens })
        .unwrap()
        .with_result(&result)
        .unwrap();
    let raw = record.to_json().unwrap();
    assert_eq!(GrantRecord::from_json(&raw).unwrap(), record);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&raw).unwrap()["result"]["expiresIn"],
        serde_json::Value::Null
    );
    for (field, value) in [
        ("expiresIn", json!(3600)),
        ("expiresAt", json!("2100-01-01T00:00:00Z")),
    ] {
        let mut bad: serde_json::Value = serde_json::from_str(&raw).unwrap();
        bad["result"][field] = value;
        assert!(GrantRecord::from_json(&bad.to_string()).is_err());
    }
    let mut bad: serde_json::Value = serde_json::from_str(&raw).unwrap();
    bad["result"]["material"]["lobsterai"]["generation"] = json!(9);
    assert!(GrantRecord::from_json(&bad.to_string()).is_err());
}

#[tokio::test]
async fn lobsterai_extension_preserves_legacy_result_json_and_material_ids() {
    let f = LocalFixture::new("https://example.com/token").await;
    let req = request(&f);
    let result = req
        .result(
            "next",
            Some("next-refresh"),
            3600,
            "2100-01-01T00:00:00Z",
            None,
        )
        .unwrap();
    let record = GrantRecord::sending(&req).with_result(&result).unwrap();
    assert_eq!(
        serde_json::to_string(&result).unwrap(),
        r#"{"material":{"endpoint":"https://example.com/token","client":"fixture-client","access":"next","refresh":"next-refresh","context":null,"expiresAt":"2100-01-01T00:00:00Z"},"expiresIn":3600,"expiresAt":"2100-01-01T00:00:00Z"}"#
    );

    let raw = record.to_json().unwrap();
    assert!(!raw.contains("lobsterai"));
    assert!(raw.contains("\"expiresIn\":3600,\"expiresAt\":\"2100-01-01T00:00:00Z\""));
    assert_eq!(
        GrantRecord::from_json(&raw).unwrap().to_json().unwrap(),
        raw
    );
    let old = serde_json::json!([
        "gateway-refresh-material-v1",
        req.material.endpoint,
        req.material.client,
        req.material.refresh,
        req.material.access,
        req.material.context,
        req.material.expires_at
    ]);
    use sha2::{Digest, Sha256};
    assert_eq!(
        req.material_id(),
        hex::encode(Sha256::digest(old.to_string()))
    );
    for field in ["expiresIn", "expiresAt"] {
        let mut bad: serde_json::Value = serde_json::from_str(&raw).unwrap();
        bad["result"][field] = serde_json::Value::Null;
        assert!(GrantRecord::from_json(&bad.to_string()).is_err());
        bad["result"].as_object_mut().unwrap().remove(field);
        assert!(GrantRecord::from_json(&bad.to_string()).is_err());
    }
}

#[tokio::test]
async fn lobsterai_config_rejects_expiry_target_and_token_binding_changes() {
    let f = fixture().await;
    let snapshot = f.runtime.route_config().snapshot();
    for change in ["expiry", "lifetime", "access", "target", "mixed"] {
        let mut doc = snapshot.document().clone();
        let c = &mut doc.providers[0].credentials[0];
        match change {
            "expiry" => c.expires_at = Some("2100-01-01T00:00:00Z".into()),
            "lifetime" => c.token_expires_in_secs = Some(3600),
            "access" => c.api_key = Some("changed".into()),
            "target" => c.base_url = Some("https://example.com/v1".into()),
            _ => c.accio_sgcookie = Some("foreign".into()),
        }
        assert!(
            f.runtime
                .commit_document(snapshot.revision().id(), doc, None)
                .await
                .is_err(),
            "{change}"
        );
    }
}
