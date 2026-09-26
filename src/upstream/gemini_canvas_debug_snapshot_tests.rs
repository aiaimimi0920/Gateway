use super::*;

#[test]
fn gemini_canvas_debug_header_adapters_bound_and_preserve_selection() {
    let input: HashMap<_, _> = (0..1000)
        .map(|n| (format!("h{n:04}"), "x".repeat(1024)))
        .collect();
    let snapshot = gemini_canvas_debug_headers_snapshot_from_hash_map(&input);
    assert_eq!(snapshot["raw"].as_object().unwrap().len(), 64);
    assert!(snapshot["raw"].get("h0063").is_some());
    assert!(snapshot["raw"].get("h0064").is_none());
    assert!(snapshot["raw"]["h0000"].as_str().unwrap().chars().count() <= 512);
    let mut headers = HeaderMap::new();
    headers.insert("cookie", "fixture-cookie".parse().unwrap());
    headers.insert("x-fixture", "safe".parse().unwrap());
    let snapshot = gemini_canvas_debug_headers_snapshot_from_header_map(&headers);
    assert_eq!(snapshot["raw"]["cookie"], "<redacted>");
    assert_eq!(snapshot["raw"]["x-fixture"], "safe");
}

#[test]
fn gemini_canvas_debug_snapshot_omits_malformed_and_deep_json() {
    for input in [
        "{\"s i d\":\"fixture-malformed\",}".to_string(),
        "[\"fixture-malformed\"".to_string(),
        format!("{}0{}", "[".repeat(32), "]".repeat(32)),
    ] {
        let snapshot = gemini_canvas_debug_form_snapshot(&[("f.req".to_string(), input)]);
        assert_eq!(snapshot["object"]["f.req"], "<omitted: diagnostic budget>");
        assert!(snapshot["parsed"].as_object().unwrap().is_empty());
    }
}

#[test]
fn gemini_canvas_debug_snapshot_keeps_duplicate_order_and_quoted_brackets() {
    let fields = vec![
        ("q".to_string(), "first".to_string()),
        ("q".to_string(), "second".to_string()),
        (
            "encoded".to_string(),
            json!({"text": "[".repeat(100)}).to_string(),
        ),
    ];
    let snapshot = gemini_canvas_debug_query_snapshot(&fields);
    assert_eq!(snapshot["items"][0]["value"], "first");
    assert_eq!(snapshot["items"][1]["value"], "second");
    assert_eq!(snapshot["object"]["q"], "second");
    let encoded: Value =
        serde_json::from_str(snapshot["object"]["encoded"].as_str().unwrap()).unwrap();
    // The inner string looks like malformed JSON and is omitted, not treated as parser nesting.
    assert!(encoded.is_object());
    assert_eq!(encoded["text"], "<omitted: diagnostic budget>");
}

#[test]
fn gemini_canvas_debug_snapshot_redacts_credentials_in_every_projection() {
    let nested = json!({"access_token": "fixture-nested", "hello": "world"}).to_string();
    let form = vec![
        ("at".to_string(), "fixture-at".to_string()),
        ("f.req".to_string(), json!([null, nested]).to_string()),
    ];
    let snapshot = gemini_canvas_debug_form_snapshot(&form);
    let text = snapshot.to_string();
    assert!(!text.contains("fixture-at"));
    assert!(!text.contains("fixture-nested"));
    assert_eq!(snapshot["parsed"]["f.req.inner"]["hello"], "world");
    assert_eq!(form[0].1, "fixture-at");
    let query = gemini_canvas_debug_query_snapshot(&form);
    assert!(!query.to_string().contains("fixture-"));
}

#[test]
fn gemini_canvas_debug_snapshot_redacts_additional_headers_and_jspb() {
    let pairs = vec![
        (
            "Proxy-Authorization".to_string(),
            "fixture-proxy".to_string(),
        ),
        ("X-Goog-Api-Key".to_string(), "fixture-api".to_string()),
        ("Set-Cookie".to_string(), "fixture-cookie".to_string()),
        (
            "x-goog-ext-73010989-jspb".to_string(),
            json!({"token": "fixture-jspb"}).to_string(),
        ),
    ];
    assert!(!gemini_canvas_debug_headers_snapshot_from_pairs(&pairs)
        .to_string()
        .contains("fixture-"));
}

#[test]
fn gemini_canvas_debug_snapshot_bounds_fields_and_large_encoded_input() {
    let fields: Vec<_> = (0..1000)
        .map(|i| (format!("field{i}"), "x".repeat(1024)))
        .collect();
    let snapshot = gemini_canvas_debug_query_snapshot(&fields);
    assert!(snapshot["items"].as_array().unwrap().len() <= 64);
    assert!(
        snapshot["items"][0]["value"]
            .as_str()
            .unwrap()
            .chars()
            .count()
            <= 512
    );
    let huge = format!("{{\"token\":\"{}\"}}", "fixture-large".repeat(10000));
    let snapshot = gemini_canvas_debug_form_snapshot(&[("f.req".to_string(), huge)]);
    assert!(!snapshot.to_string().contains("fixture-large"));
    assert!(snapshot.to_string().len() < 2048);
}

#[test]
fn gemini_canvas_debug_headers_snapshot_redacts_sensitive_values_and_parses_jspb_json() {
    let snapshot = gemini_canvas_debug_headers_snapshot_from_pairs(&[
        ("cookie".to_string(), "SID=secret".to_string()),
        ("authorization".to_string(), "Bearer secret".to_string()),
        (
            "x-goog-ext-73010989-jspb".to_string(),
            "{\"foo\":1}".to_string(),
        ),
    ]);

    assert_eq!(snapshot["raw"]["cookie"], "<redacted>");
    assert_eq!(snapshot["raw"]["authorization"], "<redacted>");
    assert_eq!(snapshot["parsed"]["x-goog-ext-73010989-jspb"]["foo"], 1);
}

#[test]
fn gemini_canvas_debug_form_snapshot_parses_f_req_outer_and_inner_payload() {
    let outer = serde_json::to_string(&json!([null, "{\"hello\":\"world\"}"]))
        .expect("serialize f.req outer");
    let snapshot = gemini_canvas_debug_form_snapshot(&[
        ("f.req".to_string(), outer),
        ("at".to_string(), "token".to_string()),
    ]);

    assert_eq!(snapshot["object"]["at"], "<redacted>");
    assert_eq!(
        snapshot["parsed"]["f.req.outer"][1],
        "{\"hello\":\"world\"}"
    );
    assert_eq!(snapshot["parsed"]["f.req.inner"]["hello"], "world");
}
