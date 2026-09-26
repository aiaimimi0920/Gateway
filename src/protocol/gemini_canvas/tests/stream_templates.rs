use super::*;

#[test]
fn build_text_stream_generate_request_from_template_reuses_envelope() {
    let storage_state = json!({
        "cookies": [],
        "origins": [],
        "textStreamGenerateTemplate": {
            "url": "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate?bl=bl-1&f.sid=sid-1&hl=zh-CN&_reqid=12345&rt=c",
            "headers": {
                "origin": "https://gemini.google.com",
                "referer": "https://gemini.google.com/",
                "x-goog-ext-525001261-jspb": "[1,null,null,null,\"fbb127bbb056c959\",null,null,0,[4],null,null,1,null,null,1,null,\"HEADER-ID\"]",
                "x-goog-ext-525005358-jspb": "[\"REQ-ID\",1]"
            },
            "postData": "f.req=%5Bnull%2C%22%5B%5B%5C%22template%20prompt%5C%22%2C0%2Cnull%2Cnull%2Cnull%2Cnull%2C0%5D%2C%5B%5C%22zh-CN%5C%22%5D%2C%5B%5C%22%5C%22%2C%5C%22%5C%22%2C%5C%22%5C%22%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5C%22%5C%22%5D%2C%5C%22opaque-state-1%5C%22%2C%5C%22deadbeefdeadbeefdeadbeefdeadbeef%5C%22%2Cnull%2C%5B0%5D%2C1%2Cnull%2Cnull%2C1%2C0%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5B%5B0%5D%5D%2C0%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C1%2Cnull%2Cnull%2C%5B4%5D%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5B2%5D%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C0%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5C%22REQ-ID%5C%22%2Cnull%2C%5B%5D%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C1%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C1%5D%22%5D&"
        }
    });

    let request = build_text_stream_generate_request_from_template(
        &storage_state,
        "Reply with exactly: templated ok",
    )
    .unwrap()
    .unwrap();

    assert_eq!(
            request.url,
            "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate"
        );
    assert_eq!(
        request
            .query
            .iter()
            .find(|(key, _)| key == "bl")
            .map(|(_, value)| value.as_str()),
        Some("bl-1")
    );
    assert_eq!(
        request
            .headers
            .get("x-goog-ext-525005358-jspb")
            .map(String::as_str),
        Some("[\"REQ-ID\",1]")
    );
    let f_req = request
        .form
        .iter()
        .find(|(key, _)| key == "f.req")
        .map(|(_, value)| value)
        .unwrap();
    let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
    let inner = serde_json::from_str::<Vec<Value>>(outer[1].as_str().unwrap()).unwrap();
    assert_eq!(
        inner[0]
            .as_array()
            .and_then(|items| items.first())
            .and_then(Value::as_str),
        Some("Reply with exactly: templated ok")
    );
    assert_eq!(inner[3].as_str(), Some("opaque-state-1"));
    assert_eq!(inner[59].as_str(), Some("REQ-ID"));
    assert!(inner[49].is_null());
}

#[test]
fn build_image_stream_generate_request_from_template_reuses_envelope_and_refreshes_request_uuid() {
    let storage_state = json!({
        "cookies": [],
        "origins": [],
        "imageStreamGenerateTemplate": {
            "url": "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate?bl=bl-1&f.sid=sid-1&hl=zh-CN&_reqid=12345&rt=c",
            "headers": {
                "referer": "https://gemini.google.com/",
                "x-goog-ext-525001261-jspb": "[1,null,null,null,\"56fdd199312815e2\",null,null,0,[4],null,null,2,null,null,1,null,\"HEADER-ID\"]",
                "x-goog-ext-525005358-jspb": "[\"REQ-ID\",1]",
                "x-goog-ext-73010989-jspb": "[0]",
                "x-goog-ext-73010990-jspb": "[0]",
                "x-same-domain": "1"
            },
            "postData": "f.req=%5Bnull%2C%22%5B%5B%5C%22template%20image%20prompt%5C%22%2C0%2Cnull%2Cnull%2Cnull%2Cnull%2C0%5D%2C%5B%5C%22zh-CN%5C%22%5D%2C%5B%5C%22%5C%22%2C%5C%22%5C%22%2C%5C%22%5C%22%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5C%22%5C%22%5D%2C%5C%22opaque-image-state-1%5C%22%2C%5C%22deadbeefdeadbeefdeadbeefdeadbeef%5C%22%2Cnull%2C%5B0%5D%2C1%2Cnull%2Cnull%2C1%2C0%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5B%5B0%5D%5D%2C0%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C1%2Cnull%2Cnull%2C%5B4%5D%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C14%2Cnull%2Cnull%2Cnull%2C0%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5C%22REQ-ID%5C%22%2Cnull%2C%5B%5D%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C1%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C1%5D%22%5D&at=AT-TOKEN"
        }
    });

    let request = build_image_stream_generate_request_from_template(
        &storage_state,
        "Reply with exactly: templated image ok",
        "NEW-REQ-ID",
    )
    .unwrap()
    .unwrap();

    assert_eq!(
            request.url,
            "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate"
        );
    assert_eq!(
        request
            .headers
            .get("x-goog-ext-525005358-jspb")
            .map(String::as_str),
        Some("[\"NEW-REQ-ID\",1]")
    );
    assert!(request.raw_post_data.contains("f.req="));
    assert!(request.raw_post_data.contains("at=AT-TOKEN"));
    assert!(request
        .form
        .iter()
        .any(|(key, value)| key == "at" && value == "AT-TOKEN"));
    let f_req = request
        .form
        .iter()
        .find(|(key, _)| key == "f.req")
        .map(|(_, value)| value)
        .unwrap();
    let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
    let inner = serde_json::from_str::<Vec<Value>>(outer[1].as_str().unwrap()).unwrap();
    assert_eq!(
        inner[0]
            .as_array()
            .and_then(|items| items.first())
            .and_then(Value::as_str),
        Some("Reply with exactly: templated image ok")
    );
    assert_eq!(inner[3].as_str(), Some("opaque-image-state-1"));
    assert_eq!(inner[59].as_str(), Some("NEW-REQ-ID"));
}

#[test]
fn build_image_stream_generate_request_from_template_keeps_original_request_uuid_when_empty() {
    let storage_state = json!({
        "cookies": [],
        "origins": [],
        "imageStreamGenerateTemplate": {
            "url": "https://gemini.google.com/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate?bl=bl-1&f.sid=sid-1&hl=zh-CN&_reqid=12345&rt=c",
            "headers": {
                "x-goog-ext-525005358-jspb": "[\"REQ-ID\",1]"
            },
            "postData": "f.req=%5Bnull%2C%22%5B%5B%5C%22template%20image%20prompt%5C%22%2C0%2Cnull%2Cnull%2Cnull%2Cnull%2C0%5D%2C%5B%5C%22zh-CN%5C%22%5D%2C%5B%5C%22%5C%22%2C%5C%22%5C%22%2C%5C%22%5C%22%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5C%22%5C%22%5D%2C%5C%22opaque-image-state-1%5C%22%2C%5C%22deadbeefdeadbeefdeadbeefdeadbeef%5C%22%2Cnull%2C%5B0%5D%2C1%2Cnull%2Cnull%2C1%2C0%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5B%5B0%5D%5D%2C0%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C1%2Cnull%2Cnull%2C%5B4%5D%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C14%2Cnull%2Cnull%2Cnull%2C0%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5C%22REQ-ID%5C%22%2Cnull%2C%5B%5D%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C1%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C1%5D%22%5D&at=AT-TOKEN"
        }
    });

    let request = build_image_stream_generate_request_from_template(
        &storage_state,
        "Reply with exactly: templated image ok",
        "",
    )
    .unwrap()
    .unwrap();

    assert_eq!(
        request
            .headers
            .get("x-goog-ext-525005358-jspb")
            .map(String::as_str),
        Some("[\"REQ-ID\",1]")
    );
    let f_req = request
        .form
        .iter()
        .find(|(key, _)| key == "f.req")
        .map(|(_, value)| value)
        .unwrap();
    let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
    let inner = serde_json::from_str::<Vec<Value>>(outer[1].as_str().unwrap()).unwrap();
    assert_eq!(
        inner[0]
            .as_array()
            .and_then(|items| items.first())
            .and_then(Value::as_str),
        Some("Reply with exactly: templated image ok")
    );
}

#[test]
fn harvest_image_edit_stream_generate_seed_reads_template_slots() {
    let storage_state = json!({
        "imageEditStreamGenerateTemplate": {
            "postData": "f.req=%5Bnull%2C%22%5B%5B%5C%22template%20image%20prompt%5C%22%2C0%2Cnull%2C%5B%5B%5B%5C%22%2Fcontrib_service%2Fttl_1d%2Fexample%5C%22%2C1%2Cnull%2C%5C%22image%2Fjpeg%5C%22%5D%2C%5C%22edit-source.jpg%5C%22%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5B0%5D%5D%5D%2Cnull%2Cnull%2C0%5D%2C%5B%5C%22zh-CN%5C%22%5D%2C%5B%5C%22%5C%22%2C%5C%22%5C%22%2C%5C%22%5C%22%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5C%22%5C%22%5D%2C%5C%22seed-opaque-state%5C%22%2C%5C%220123456789abcdef0123456789abcdef%5C%22%2Cnull%2C%5B1%5D%2C1%2Cnull%2Cnull%2C1%2C0%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5B%5B0%5D%5D%2C0%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C1%2Cnull%2Cnull%2C%5B4%5D%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5B1%5D%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C14%2Cnull%2Cnull%2Cnull%2C0%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C%5C%22seed-request-id%5C%22%2Cnull%2C%5B%5D%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C2%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2Cnull%2C1%5D%22%5D&at=AT-TOKEN"
        }
    });

    let seed = harvest_image_edit_stream_generate_seed(&storage_state).unwrap();
    assert_eq!(seed.opaque_state.as_deref(), Some("seed-opaque-state"));
    assert_eq!(
        seed.request_hex.as_deref(),
        Some("0123456789abcdef0123456789abcdef")
    );
    assert_eq!(seed.request_uuid.as_deref(), Some("seed-request-id"));
}
