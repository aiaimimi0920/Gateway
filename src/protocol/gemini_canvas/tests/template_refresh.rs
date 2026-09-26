use super::*;

#[test]
fn refresh_stream_generate_template_with_bootstrap_replaces_query_and_at() {
    let template = GeminiCanvasTextStreamGenerateTemplate {
            url: "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate?bl=old-bl&f.sid=old-sid&hl=en-US&_reqid=12345&rt=c".to_string(),
            query: vec![
                ("bl".to_string(), "old-bl".to_string()),
                ("f.sid".to_string(), "old-sid".to_string()),
                ("hl".to_string(), "en-US".to_string()),
                ("_reqid".to_string(), "12345".to_string()),
                ("rt".to_string(), "c".to_string()),
            ],
            form: vec![
                ("f.req".to_string(), "[null,\"[]\"]".to_string()),
                ("at".to_string(), "old-at".to_string()),
            ],
            raw_post_data: "f.req=%5Bnull%2C%22%5B%5D%22%5D&at=old-at".to_string(),
            headers: HashMap::new(),
        };
    let bootstrap = gemini_web::GeminiWebBootstrap {
        access_token: Some("new-at".to_string()),
        build_label: Some("new-bl".to_string()),
        session_id: Some("new-sid".to_string()),
        language: "zh-CN".to_string(),
        push_id: None,
        client_pctx: None,
        app_page_path: None,
    };

    let refreshed =
        refresh_stream_generate_template_with_bootstrap(&template, &bootstrap, true).unwrap();
    assert_eq!(
            refreshed.url,
            "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate"
        );
    assert_eq!(
        refreshed
            .query
            .iter()
            .find(|(key, _)| key == "bl")
            .map(|(_, value)| value.as_str()),
        Some("new-bl")
    );
    assert_eq!(
        refreshed
            .query
            .iter()
            .find(|(key, _)| key == "f.sid")
            .map(|(_, value)| value.as_str()),
        Some("new-sid")
    );
    assert_eq!(
        refreshed
            .query
            .iter()
            .find(|(key, _)| key == "hl")
            .map(|(_, value)| value.as_str()),
        Some("zh-CN")
    );
    assert_eq!(
        refreshed
            .form
            .iter()
            .find(|(key, _)| key == "at")
            .map(|(_, value)| value.as_str()),
        Some("new-at")
    );
    assert!(refreshed.raw_post_data.contains("at=new-at"));
}

#[test]
fn refresh_stream_generate_template_access_token_replaces_only_at() {
    let template = GeminiCanvasTextStreamGenerateTemplate {
            url: "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate".to_string(),
            query: vec![
                ("bl".to_string(), "old-bl".to_string()),
                ("f.sid".to_string(), "old-sid".to_string()),
            ],
            form: vec![
                ("f.req".to_string(), "[null,\"[]\"]".to_string()),
                ("at".to_string(), "old-at".to_string()),
            ],
            raw_post_data: "f.req=%5Bnull%2C%22%5B%5D%22%5D&at=old-at".to_string(),
            headers: HashMap::from([("accept".to_string(), "*/*".to_string())]),
        };

    let refreshed = refresh_stream_generate_template_access_token(&template, "new-at");
    assert_eq!(refreshed.url, template.url);
    assert_eq!(refreshed.query, template.query);
    assert_eq!(
        refreshed
            .form
            .iter()
            .find(|(key, _)| key == "at")
            .map(|(_, value)| value.as_str()),
        Some("new-at")
    );
    assert!(refreshed.raw_post_data.contains("at=new-at"));
    assert_eq!(refreshed.headers, template.headers);
}

#[test]
fn refresh_stream_generate_template_model_header_id_rewrites_suffix_uuid() {
    let mut template = GeminiCanvasTextStreamGenerateTemplate {
            url: "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate".to_string(),
            query: vec![],
            form: vec![],
            raw_post_data: "f.req=[]".to_string(),
            headers: HashMap::from([(
                "x-goog-ext-525001261-jspb".to_string(),
                "[1,null,null,null,\"56fdd199312815e2\",null,null,0,[4],null,null,2,null,null,1,null,\"OLD-HEADER-ID\"]".to_string(),
            )]),
        };

    assert!(refresh_stream_generate_template_model_header_id(
        &mut template,
        "NEW-HEADER-ID"
    ));

    let parsed = serde_json::from_str::<Vec<Value>>(
        template
            .headers
            .get("x-goog-ext-525001261-jspb")
            .expect("model header"),
    )
    .expect("parsed model header");
    assert_eq!(parsed[16].as_str(), Some("NEW-HEADER-ID"));
}

#[test]
fn refresh_stream_generate_template_request_hex_rewrites_inner_slot() {
    let mut template = GeminiCanvasTextStreamGenerateTemplate {
            url: "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate".to_string(),
            query: vec![],
            form: vec![(
                "f.req".to_string(),
                "[null,\"[[\\\"prompt\\\",0,null,null,null,null,0],[\\\"zh-CN\\\"],[\\\"\\\",\\\"\\\",\\\"\\\",null,null,null,null,null,null,\\\"\\\"],\\\"opaque\\\",\\\"0123456789abcdef0123456789abcdef\\\",null,[1],1]\"]".to_string(),
            )],
            raw_post_data: "f.req=%5Bnull%2C%22%5B%5B%5C%22prompt%5C%22%2C0%2Cnull%2Cnull%2Cnull%2Cnull%2C0%5D%2C%5B%5C%22zh-CN%5C%22%5D%2C%5B%5C%22%5C%22%2C%5C%22%5C%22%2C%5C%22%5C%22%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5C%22%5C%22%5D%2C%5C%22opaque%5C%22%2C%5C%220123456789abcdef0123456789abcdef%5C%22%2Cnull%2C%5B1%5D%2C1%5D%22%5D".to_string(),
            headers: HashMap::new(),
        };

    assert!(refresh_stream_generate_template_request_hex(
        &mut template,
        "fedcba9876543210fedcba9876543210"
    ));

    let f_req = template
        .form
        .iter()
        .find(|(key, _)| key == "f.req")
        .map(|(_, value)| value)
        .expect("f.req");
    let outer = serde_json::from_str::<Vec<Value>>(f_req).expect("outer");
    let inner =
        serde_json::from_str::<Vec<Value>>(outer[1].as_str().expect("inner json")).expect("inner");
    assert_eq!(inner[4].as_str(), Some("fedcba9876543210fedcba9876543210"));
    assert!(template
        .raw_post_data
        .contains("fedcba9876543210fedcba9876543210"));
}
