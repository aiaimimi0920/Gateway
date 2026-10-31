use super::*;

fn candidate(rcid: &str, status: i64) -> Value {
    let mut data = vec![Value::Null; 13];
    data[0] = json!(rcid);
    data[8] = json!([status]);
    data[12] = json!([
        null,
        null,
        null,
        null,
        null,
        null,
        null,
        [[[[
            null,
            null,
            null,
            [
                null,
                null,
                "fixture",
                "https://lh3.googleusercontent.com/fixture.png"
            ]
        ]]]]
    ]);
    json!(data)
}

fn stream(cid: &str, rid: &str, rcid: &str) -> Value {
    json!([
        "wrb.fr",
        null,
        json!([null, [cid, rid], null, null, [candidate(rcid, 1)]]).to_string()
    ])
}

fn accepted() -> AcceptedImageTurn {
    accepted_image_turn(&json!([stream("c_fixture", "r_fixture", "rc_fixture")]).to_string())
        .unwrap()
}

fn history(turns: Value) -> String {
    json!([[
        "wrb.fr",
        IMAGE_READ_RPC,
        json!([turns]).to_string(),
        null,
        null,
        null,
        "generic"
    ]])
    .to_string()
}

fn turn(rid: &str, candidates: Value) -> Value {
    json!([["c_fixture", rid], null, null, [candidates]])
}

#[test]
fn binding_is_structural_complete_and_unambiguous() {
    let binding = accepted();
    assert_eq!(binding.conversation, "c_fixture");
    assert_eq!(binding.response, "r_fixture");
    assert_eq!(binding.candidate, "rc_fixture");
    for body in [
        "c_fixture r_fixture rc_fixture https://lh3.googleusercontent.com/old.png".into(),
        json!({"nested":[stream("c_fixture","r_fixture","rc_fixture")]}).to_string(),
        json!([
            stream("c_fixture", "r_fixture", "rc_fixture"),
            stream("c_other", "r_other", "rc_other")
        ])
        .to_string(),
        json!([
            stream("c_fixture", "r_fixture", "rc_fixture"),
            stream("c_fixture", "r_fixture", "rc_other")
        ])
        .to_string(),
        json!([stream("c_fixture/evil", "r_fixture", "rc_fixture")]).to_string(),
        json!([[
            "wrb.fr",
            null,
            json!([null, ["c_fixture", "r_fixture"]]).to_string()
        ]])
        .to_string(),
        json!([[
            "wrb.fr",
            "hNvQHb",
            json!([
                null,
                ["c_fixture", "r_fixture"],
                null,
                null,
                [candidate("rc_fixture", 1)]
            ])
            .to_string()
        ]])
        .to_string(),
    ] {
        assert!(accepted_image_turn(&body).is_err());
    }
}

#[test]
fn recovery_never_chooses_latest_turn_or_another_candidate() {
    let binding = accepted();
    for turns in [
        json!([turn("r_old", json!([candidate("rc_fixture", 2)]))]),
        json!([turn("r_fixture", json!([candidate("rc_other", 2)]))]),
        json!([
            turn("r_old", json!([candidate("rc_fixture", 2)])),
            turn("r_fixture", json!([candidate("rc_fixture", 1)]))
        ]),
    ] {
        assert!(recovered_image_assets(&history(turns), &binding)
            .unwrap()
            .is_none());
    }
    let assets = recovered_image_assets(
        &history(json!([
            turn("r_new", json!([candidate("rc_new", 2)])),
            turn("r_fixture", json!([candidate("rc_fixture", 2)]))
        ])),
        &binding,
    )
    .unwrap()
    .unwrap();
    assert_eq!(assets.len(), 1);
    assert_eq!(
        assets[0].url,
        "https://lh3.googleusercontent.com/fixture.png"
    );
}

#[test]
fn only_finished_candidate_with_generated_image_fields_is_a_result() {
    let binding = accepted();
    for status in [0, 3, -1] {
        assert!(recovered_image_assets(
            &history(json!([turn(
                "r_fixture",
                json!([candidate("rc_fixture", status)])
            )])),
            &binding
        )
        .is_err());
    }
    let mut value = candidate("rc_fixture", 2);
    value[12] = Value::Null;
    value[1] = json!(["https://lh3.googleusercontent.com/not-generated.png"]);
    assert!(recovered_image_assets(
        &history(json!([turn("r_fixture", json!([value]))])),
        &binding
    )
    .unwrap()
    .is_none());
}

#[test]
fn duplicate_conflicting_or_error_results_fail_closed() {
    let binding = accepted();
    let result = turn("r_fixture", json!([candidate("rc_fixture", 2)]));
    let mut contradictory = result.clone();
    contradictory[0][0] = json!("c_other");
    for turns in [
        json!([result.clone(), result]),
        json!([contradictory]),
        json!([turn(
            "r_fixture",
            json!([candidate("rc_fixture", 2), candidate("rc_fixture", 2)])
        )]),
    ] {
        assert!(recovered_image_assets(&history(turns), &binding).is_err());
    }
    let good = history(json!([turn(
        "r_fixture",
        json!([candidate("rc_fixture", 2)])
    )]));
    let rows: Value = serde_json::from_str(&good).unwrap();
    for change in [json!("EqPOKe"), json!(null)] {
        let mut rows = rows.clone();
        rows[0][1] = change;
        assert!(recovered_image_assets(&rows.to_string(), &binding).is_err());
    }
    let mut error = rows[0].clone();
    error[5] = json!(["private fixture"]);
    for body in [
        json!([rows[0], rows[0]]).to_string(),
        json!([error]).to_string(),
    ] {
        assert!(recovered_image_assets(&body, &binding).is_err());
    }
}

#[test]
fn length_framing_is_complete_utf16_and_bounded() {
    let binding = accepted();
    let mut value = candidate("rc_fixture", 2);
    value[1] = json!(["巴黎😀"]);
    let body = history(json!([turn("r_fixture", json!([value]))]));
    let framed = format!(")]}}'\n{}\n{}\n", body.encode_utf16().count(), body);
    assert!(recovered_image_assets(&framed, &binding).unwrap().is_some());
    for bad in [
        format!("{}\n{}", body.encode_utf16().count() + 1, body),
        format!("{}\n{}", body.encode_utf16().count() - 1, body),
        format!("{framed}garbage"),
        "[".repeat(IMAGE_READ_LIMIT + 1),
    ] {
        assert!(recovered_image_assets(&bad, &binding).is_err());
    }
}

#[test]
fn recovered_urls_cannot_be_arbitrary_download_targets() {
    for url in [
        "http://lh3.googleusercontent.com/fixture.png",
        "https://evil.invalid/fixture.png",
        "https://googleusercontent.com.evil.invalid/image",
        "https://user:secret@lh3.googleusercontent.com/image",
        "https://lh3.googleusercontent.com:444/image",
        "https://127.0.0.1/image",
    ] {
        let mut value = candidate("rc_fixture", 2);
        value[12][7][0][0][0][3][3] = json!(url);
        assert!(recovered_image_assets(
            &history(json!([turn("r_fixture", json!([value]))])),
            &accepted()
        )
        .is_err());
    }
}

#[test]
fn read_request_contains_only_bound_cid_and_preserves_abort_privacy() {
    let bootstrap = gemini_web::GeminiWebBootstrap {
        access_token: Some("fixture".into()),
        build_label: None,
        session_id: None,
        language: "en".into(),
        push_id: None,
        client_pctx: None,
        app_page_path: Some("/app".into()),
    };
    let request = image_history_request(&accepted(), &bootstrap).unwrap();
    let wrapped: Value =
        serde_json::from_str(&request.form.iter().find(|(k, _)| k == "f.req").unwrap().1).unwrap();
    assert_eq!(wrapped[0][0][0], IMAGE_READ_RPC);
    let args: Value = serde_json::from_str(wrapped[0][0][1].as_str().unwrap()).unwrap();
    assert_eq!(args, json!(["c_fixture", 10, null, 1, [1], [4], null, 1]));
    let error = accepted_image_turn("private prompt cookie=fixture")
        .err()
        .unwrap();
    assert!(!error.message.contains("fixture"));
    assert!(!error.retryable);
    assert!(matches!(error.fallback_hint, FallbackHint::Abort { .. }));
}
