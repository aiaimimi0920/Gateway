use super::{test_support::*, *};

#[tokio::test]
async fn qwen_native_directory_accepts_the_full_2048_entry_103_page_boundary() {
    let replies = (1..=MAX_PAGES)
        .map(|number| {
            let start = (number - 1) * PAGE_SIZE;
            reply(page(
                number,
                MAX_ENTRIES,
                (start..(start + PAGE_SIZE).min(MAX_ENTRIES))
                    .map(row)
                    .collect(),
            ))
        })
        .collect();
    let server = Server::start(replies).await;
    let catalog = server.fetch().await.unwrap();
    assert_eq!(catalog.models.len(), MAX_ENTRIES);
    assert_eq!(catalog.models.last().unwrap(), "qwen-contract-2047");
    assert_eq!(server.calls.lock().unwrap().len(), 103);
}

#[tokio::test]
async fn qwen_native_pages_keep_exact_queries_and_workspace_without_generation() {
    let mut first: Vec<_> = (0..20).map(row).collect();
    first[1]["provider"] = serde_json::json!("other-provider");
    first[2]["capabilities"] = serde_json::json!(["IG"]);
    first[3]["inference_metadata"]["response_modality"] = serde_json::json!(["Image"]);
    first[4]
        .as_object_mut()
        .unwrap()
        .remove("inference_metadata");
    // Official examples omit provider/capabilities; Text input/output remains explicit.
    first[5].as_object_mut().unwrap().remove("provider");
    first[5].as_object_mut().unwrap().remove("capabilities");
    let server = Server::start(vec![
        reply(page(1, 21, first)),
        reply(page(2, 21, vec![row(20)])),
    ])
    .await;
    let catalog = server.fetch().await.unwrap();
    assert_eq!(catalog.models.len(), 17);
    assert!(catalog.models.contains(&"qwen-contract-5".to_owned()));
    assert_eq!(catalog.models.last().unwrap(), "qwen-contract-20");
    assert_eq!(catalog.account_base_url.as_deref(), Some(BASE));
    assert_eq!(
        catalog.source_url,
        "https://dashscope-intl.aliyuncs.com/api/v1/models"
    );
    catalog.validate(Some("qwen-official-api"), BASE).unwrap();
    let calls = server.calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    for (index, (headers, uri)) in calls.iter().enumerate() {
        assert_eq!(headers["authorization"], "Bearer synthetic-qwen-key");
        assert_eq!(uri.path(), "/api/v1/models");
        let query: Vec<_> = url::form_urlencoded::parse(uri.query().unwrap().as_bytes()).collect();
        let values = |name: &str| {
            query
                .iter()
                .filter(|(key, _)| key == name)
                .map(|(_, value)| value.as_ref())
                .collect::<Vec<_>>()
        };
        assert_eq!(values("page_no"), [(index + 1).to_string()]);
        assert_eq!(values("page_size"), ["20"]);
        assert_eq!(values("providers"), ["qwen"]);
        assert_eq!(values("supports"), ["inference"]);
        assert_eq!(values("capabilities"), ["TG", "Reasoning"]);
        assert_eq!(query.len(), 6);
    }
    let json = serde_json::to_string(&catalog).unwrap();
    assert!(!json.contains("synthetic-qwen-key") && !json.contains("verified"));
}

#[test]
fn qwen_endpoint_descriptor_has_no_default_region_or_untrusted_url_parts() {
    for region in [
        "cn-beijing",
        "ap-southeast-1",
        "ap-northeast-1",
        "eu-central-1",
        "us-east-1",
        "cn-hongkong",
    ] {
        let base = format!("https://ws-123.{region}.maas.aliyuncs.com/compatible-mode/v1");
        let endpoint = QwenEndpoint::parse(&base).unwrap();
        assert_eq!(endpoint.base_url, base);
        assert_eq!(QwenEndpoint::parse(&format!("{base}/")).unwrap(), endpoint);
    }
    for base in [
        "",
        "https://dashscope.aliyuncs.com/compatible-mode/v1",
        "https://coding.dashscope.aliyuncs.com/v1",
        "https://trial.cn-beijing.maas.aliyuncs.com/compatible-mode/v1",
        "https://ws.cn-beijing.maas.aliyuncs.com.evil.invalid/compatible-mode/v1",
        "http://ws.cn-beijing.maas.aliyuncs.com/compatible-mode/v1",
        "https://ws.cn-beijing.maas.aliyuncs.com:443/compatible-mode/v1",
        "https://user@ws.cn-beijing.maas.aliyuncs.com/compatible-mode/v1",
        "https://ws.cn-beijing.maas.aliyuncs.com/compatible-mode/v1?key=x",
        "https://ws.cn-beijing.maas.aliyuncs.com/compatible-mode/v1#fragment",
        "https://ws.cn-beijing.maas.aliyuncs.com/compatible-mode/v1//",
        "https://ws.other-region.maas.aliyuncs.com/compatible-mode/v1",
        "https://.cn-beijing.maas.aliyuncs.com/compatible-mode/v1",
        "https://a.b.cn-beijing.maas.aliyuncs.com/compatible-mode/v1",
        "https://-ws.cn-beijing.maas.aliyuncs.com/compatible-mode/v1",
        "https://ws-.cn-beijing.maas.aliyuncs.com/compatible-mode/v1",
    ] {
        assert!(QwenEndpoint::parse(base).is_none(), "{base}");
    }
    let long_label = format!(
        "https://{}.cn-beijing.maas.aliyuncs.com/compatible-mode/v1",
        "a".repeat(64)
    );
    assert!(QwenEndpoint::parse(&long_label).is_none());
}

#[tokio::test]
async fn public_qwen_catalog_rejects_wrong_products_and_invalid_keys_without_network() {
    for base in [
        "http://127.0.0.1:1",
        "https://coding.dashscope.aliyuncs.com/v1",
        "https://chat.qwen.ai",
    ] {
        assert!(fetch_catalog(base, "synthetic-only").await.is_err());
    }
    for key in [
        "".to_owned(),
        "bad\r\nkey".to_owned(),
        "é".to_owned(),
        "x".repeat(16385),
    ] {
        let error = fetch_catalog(BASE, &key).await.unwrap_err();
        assert!(!error.to_string().contains(&format!("key={key}")));
    }
}
