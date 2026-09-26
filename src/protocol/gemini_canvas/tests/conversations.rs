use super::*;

#[test]
fn build_conversation_list_probe_request_matches_capture_payload() {
    let bootstrap = gemini_web::GeminiWebBootstrap {
        access_token: Some("AT".to_string()),
        build_label: Some("boq".to_string()),
        session_id: Some("123".to_string()),
        language: "zh-CN".to_string(),
        push_id: None,
        client_pctx: None,
        app_page_path: None,
    };

    let request = build_conversation_list_probe_request(&bootstrap, "/app").unwrap();
    assert!(request
        .query
        .iter()
        .any(|(key, value)| key == "rpcids" && value == "MaZiqc"));
    assert!(request
        .query
        .iter()
        .any(|(key, value)| key == "source-path" && value == "/app"));
    let f_req = request
        .form
        .iter()
        .find(|(key, _)| key == "f.req")
        .map(|(_, value)| value)
        .unwrap();
    assert_eq!(
        f_req,
        "[[[\"MaZiqc\",\"[13,null,[1,null,1]]\",null,\"generic\"]]]"
    );
}

#[test]
fn build_conversation_list_full_request_matches_capture_payload() {
    let bootstrap = gemini_web::GeminiWebBootstrap {
        access_token: Some("AT".to_string()),
        build_label: Some("boq".to_string()),
        session_id: Some("123".to_string()),
        language: "zh-CN".to_string(),
        push_id: None,
        client_pctx: None,
        app_page_path: None,
    };

    let request = build_conversation_list_full_request(&bootstrap, "/app").unwrap();
    assert!(request
        .query
        .iter()
        .any(|(key, value)| key == "rpcids" && value == "MaZiqc"));
    assert!(request
        .query
        .iter()
        .any(|(key, value)| key == "source-path" && value == "/app"));
    let f_req = request
        .form
        .iter()
        .find(|(key, _)| key == "f.req")
        .map(|(_, value)| value)
        .unwrap();
    assert_eq!(
        f_req,
        "[[[\"MaZiqc\",\"[13,null,[0,null,1]]\",null,\"generic\"]]]"
    );
}

#[test]
fn extract_conversation_list_entries_reads_ids_titles_and_timestamps() {
    let body = concat!(
            ")]}'\n\n",
            "2131\n",
            "[[\"wrb.fr\",\"MaZiqc\",\"[null,\\\"token\\\",[[\\\"c_da24e84ec005599a\\\",\\\"canvas live skyline image\\\\n\\\\nRequested aspect ratio: 1:1.\\\",null,null,null,[1777601930,749541000],null,null,null,2],[\\\"c_90ca192406be80bb\\\",\\\"Canvas Live Launch Video Generation\\\",null,null,null,[1777501548,491550000],[[\\\"c_90ca192406be80bb\\\",\\\"r_bc98137b61ada92c\\\"],1],null,null,2]]]\",null,null,null,\"generic\"]]\n",
            "56\n",
            "[[\"di\",121],[\"af.httprm\",120,\"1294558254015590662\",5]]\n",
            "26\n",
            "[[\"e\",4,null,null,2231]]\n"
        );

    let entries = extract_conversation_list_entries(body);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].conversation_id, "c_da24e84ec005599a");
    assert_eq!(
        entries[0].title,
        "canvas live skyline image\n\nRequested aspect ratio: 1:1."
    );
    assert_eq!(entries[0].updated_at_secs, 1777601930);
    assert_eq!(entries[0].updated_at_nanos, 749541000);
    assert_eq!(entries[0].response_id, None);
    assert_eq!(
        entries[0].app_path().as_deref(),
        Some("/app/da24e84ec005599a")
    );
    assert_eq!(entries[1].conversation_id, "c_90ca192406be80bb");
    assert_eq!(
        entries[1].response_id.as_deref(),
        Some("r_bc98137b61ada92c")
    );
}

#[test]
fn extract_stream_generate_locator_reads_response_and_conversation_ids() {
    let frame1 = serde_json::to_string(&vec![json!([
        "wrb.fr",
        null,
        "[null,[null,\"r_e0e4aa76ab2755e3\"],{\"18\":\"r_e0e4aa76ab2755e3\",\"44\":false}]"
    ])])
    .unwrap();
    let frame2 = serde_json::to_string(&vec![json!([
        "wrb.fr",
        null,
        "[null,[\"c_1004db0ef60d9dda\",\"r_e0e4aa76ab2755e3\"],null,null,[]]"
    ])])
    .unwrap();
    let body = format!(
        ")]}}'\n\n{}\n{}\n{}\n{}\n",
        frame1.encode_utf16().count(),
        frame1,
        frame2.encode_utf16().count(),
        frame2
    );

    let locator = extract_stream_generate_locator(&body).unwrap();
    assert_eq!(locator.response_id, "r_e0e4aa76ab2755e3");
    assert_eq!(locator.conversation_id, "c_1004db0ef60d9dda");
    assert_eq!(locator.app_path, "/app/1004db0ef60d9dda");
}

#[test]
fn stream_generate_parseable_frame_count_recognizes_short_ack_frames() {
    let body = concat!(
        ")]}'\n\n",
        "13\n",
        "[\"wrb.fr\",null,null,null,null,[13]]\n",
        "8\n",
        "[\"di\",71]\n",
        "38\n",
        "[\"af.httprm\",70,\"-8507264369209331675\",1]\n",
        "25\n",
        "[[\"e\",4,null,null,111]]\n"
    );
    assert!(stream_generate_parseable_frame_count(body) > 0);
}

#[test]
fn stream_generate_indicates_image_edit_async_followup_ready_for_response_id_frame() {
    let body = concat!(
            ")]}'\n\n",
            "126\n",
            "[[\"wrb.fr\",null,\"[null,[null,\\\"r_async123\\\"],{\\\"18\\\":\\\"r_async123\\\",\\\"21\\\":[\\\"token\\\"],\\\"44\\\":true}]\"]]\n"
        );
    assert!(stream_generate_indicates_image_edit_async_followup_ready(
        body
    ));
}

#[test]
fn stream_generate_indicates_image_edit_async_followup_ready_for_progress_frame() {
    let body = concat!(
        ")]}'\n\n",
        "36\n",
        "[[\"wrb.fr\",null,\"[{\\\"37\\\":[4]}]\"]]\n"
    );
    assert!(stream_generate_indicates_image_edit_async_followup_ready(
        body
    ));
}

#[test]
fn stream_generate_indicates_image_edit_async_followup_ready_ignores_short_ack_only() {
    let body = concat!(
        ")]}'\n\n",
        "13\n",
        "[[\"wrb.fr\",null,null,null,null,[13]]]\n"
    );
    assert!(!stream_generate_indicates_image_edit_async_followup_ready(
        body
    ));
}

#[test]
fn stream_generate_is_image_edit_short_ack_recognizes_async_ack_frames() {
    let body = concat!(
        ")]}'\n\n",
        "13\n",
        "[\"wrb.fr\",null,null,null,null,[13]]\n",
        "8\n",
        "[\"di\",71]\n",
        "38\n",
        "[\"af.httprm\",70,\"-8507264369209331675\",1]\n"
    );
    assert!(stream_generate_is_image_edit_short_ack(body));
}

#[test]
fn stream_generate_is_image_edit_short_ack_ignores_single_ack_frame() {
    let body = concat!(
        ")]}'\n\n",
        "13\n",
        "[[\"wrb.fr\",null,null,null,null,[13]]]\n"
    );
    assert!(!stream_generate_is_image_edit_short_ack(body));
}

#[test]
fn extract_video_generation_job_id_reads_uuid_from_pending_body() {
    let body = r#"
            )]}'
            [["wrb.fr",null,"[null,[\"c_98256aabc450ff93\",\"r_931fae4dfff02e00\"],null,null,[[\"rc_pending\",[\"正在生成视频，这可能需要几分钟时间，请稍后回来查看完成状态。\nhttp://googleusercontent.com/video_gen_chip/0\n\"],null,null,null,null,null,null,[1],\"zh\",null,null,[{\"65\":[[\"http://googleusercontent.com/video_gen_chip/0\"],\"e3136f2f-29f4-4f33-9223-271d4697c60a\"]}]]]]"]]
        "#;

    let job_id = extract_video_generation_job_id(body)
        .expect("video generation job id should be recovered from pending response");
    assert_eq!(job_id, "e3136f2f-29f4-4f33-9223-271d4697c60a");
    assert!(response_indicates_video_generation_pending(body));
}

#[test]
fn response_indicates_video_generation_pending_for_async_contract_ready_frames() {
    let body = r#"
            )]}'
            [["wrb.fr",null,"[null,[null,\"r_7013976975ffa957\"],{\"18\":\"r_7013976975ffa957\",\"21\":[\"8X5YJX-Z3wmpPmdEQeUnP4LP6yxOur647qqxisHb4vM\"],\"44\":true}]"]]
            [["wrb.fr",null,null,null,null,[8,null,[[\"type.googleapis.com/assistant.boq.bard.application.BardErrorInfo\",[1053]]]]]
        "#;

    assert!(response_indicates_video_generation_pending(body));
}

#[test]
fn build_video_metadata_followup_request_matches_capture_payload() {
    let bootstrap = gemini_web::GeminiWebBootstrap {
        access_token: Some("at-1".to_string()),
        build_label: Some("bl-1".to_string()),
        session_id: Some("sid-1".to_string()),
        language: "zh-CN".to_string(),
        push_id: None,
        client_pctx: None,
        app_page_path: None,
    };

    let request = build_video_metadata_followup_request(
        &bootstrap,
        "/app/98256aabc450ff93",
        "c_98256aabc450ff93",
        "r_931fae4dfff02e00",
    )
    .unwrap();

    assert!(request
        .query
        .iter()
        .any(|(key, value)| key == "rpcids" && value == "MUAZcd"));
    let f_req = request
        .form
        .iter()
        .find(|(key, _)| key == "f.req")
        .map(|(_, value)| value)
        .unwrap();
    let outer = serde_json::from_str::<Vec<Value>>(f_req).unwrap();
    let rpc = outer[0].as_array().and_then(|items| items.first()).unwrap();
    let rpc_items = rpc.as_array().unwrap();
    assert_eq!(rpc_items[0].as_str(), Some("MUAZcd"));

    let payload = serde_json::from_str::<Value>(rpc_items[1].as_str().unwrap()).unwrap();
    assert_eq!(
        payload,
        json!([
            null,
            [["unread_metadata"]],
            [
                "c_98256aabc450ff93",
                null,
                null,
                null,
                null,
                null,
                [["c_98256aabc450ff93", "r_931fae4dfff02e00"], 0]
            ]
        ])
    );
}
