//! Synthetic provider declarations prove protocol selection, not live model entitlement.
use super::{parse_catalog, OAuthClient, OAuthError, Site, WireProtocol};
use serde_json::json;

#[test]
fn dim_sites_and_clients_cannot_borrow_each_others_authority() {
    let cn =
        OAuthClient::new(Site::China, "permitted-cn", &["openid", "profile", "email"]).unwrap();
    let global = OAuthClient::new(Site::Global, "permitted-global", &["openid"]).unwrap();
    assert_eq!(cn.site().issuer(), "https://dimagent.cn");
    assert_eq!(global.site().issuer(), "https://dimagent.com");
    assert_ne!(cn, global);
    for scopes in [
        vec!["openid", "openid"],
        vec!["openid", "bad scope"],
        vec!["profile"],
    ] {
        assert!(OAuthClient::new(Site::China, "permitted-cn", &scopes).is_err());
    }
    assert!(OAuthClient::new(Site::China, "", &["openid"]).is_err());
}

#[test]
fn dim_catalog_uses_declared_protocol_order_not_model_name_or_price_guesses() {
    let bytes = serde_json::to_vec(&json!({"data":[
        {"id":"synthetic-chat","dim":{"displayName":"Synthetic Chat","endpoint_type":"openai",
            "limit":{"context":128000,"output":8192},"toolCalling":true,"modalities":["text","image"],
            "reasoning":{"supported":true,"effortOptions":["low","high"]}}},
        {"id":"synthetic-messages","dim":{"endpoint_type":"anthropic","tool_call":true}},
        {"id":"synthetic-responses","supported_endpoint_types":["unsupported","openai-response","openai"]},
        {"id":"synthetic-gemini","dim":{"endpoint_type":"gemini"}},
        {"id":"synthetic-default","dim":null},
        {"id":"skipped","dim":{"endpoint_type":"unknown"}},
        {"id":"unavailable","dim":{"status":"unavailable"}}
    ]})).unwrap();
    let models = parse_catalog(&bytes).unwrap();
    assert_eq!(models.len(), 5);
    assert_eq!(models[0].protocol, WireProtocol::Chat);
    assert_eq!(models[1].protocol, WireProtocol::Messages);
    assert_eq!(models[2].protocol, WireProtocol::Responses);
    assert_eq!(models[3].protocol, WireProtocol::Gemini);
    assert_eq!(models[4].protocol, WireProtocol::Chat);
    assert_eq!(models[0].context_window, Some(128000));
    assert_eq!(models[0].max_output_tokens, Some(8192));
    assert_eq!(models[0].tools, Some(true));
    assert_eq!(models[0].input_modalities, ["text", "image"]);
    assert_eq!(models[0].reasoning_efforts, ["low", "high"]);
    assert_eq!(models[4].context_window, None);
    assert_eq!(models[4].tools, None);
    assert_eq!(
        serde_json::to_value(&models)
            .unwrap()
            .to_string()
            .contains("price"),
        false
    );
}

#[test]
fn dim_catalog_rejects_ambiguous_or_invalid_declarations_without_empty_success() {
    for value in [
        json!({"data":[{"id":"same"},{"id":"same"}]}),
        json!({"data":[{"id":"m","dim":{"id":"other"}}]}),
        json!({"data":[{"id":"m","dim":{"tool_call":"true"}}]}),
        json!({"data":[{"id":"m","dim":{"endpoint_type":"unknown"}}]}),
        json!({"data":[{"id":"m","dim":{"context_window":-1}}]}),
        json!({"data":{}}),
    ] {
        assert!(parse_catalog(&serde_json::to_vec(&value).unwrap()).is_err());
    }
    assert!(parse_catalog(
        br#"{"data":[{"id":"m","dim":{"endpoint_type":"openai","endpoint_type":"anthropic"}}]}"#
    )
    .is_err());
    assert!(parse_catalog(br#"{"data":[],"data":[{"id":"m"}]}"#).is_err());
    assert!(parse_catalog(br#"{"data":[]}"#).unwrap().is_empty());
    assert!(matches!(
        parse_catalog(&vec![b' '; 2 * 1024 * 1024 + 1]),
        Err(OAuthError::ResponseTooLarge)
    ));
}
