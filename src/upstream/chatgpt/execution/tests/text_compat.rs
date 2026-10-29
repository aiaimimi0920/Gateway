//! Default text calls must not silently enter the legacy f/image prepare flow.
use super::*;
use futures::StreamExt;

#[tokio::test]
async fn text_compat_defaults_to_plain_conversation_for_buffered_and_streaming() {
    let client = Client::builder().no_proxy().build().unwrap();
    for streaming in [false, true] {
        let state = RequestState::default();
        let url = spawn_chatgpt_f_conversation_server(state.clone()).await;
        let mut payload = make_payload(&url);
        payload
            .headers
            .insert("Cookie".into(), "session=fixture".into());
        payload.default_model = Some("gpt-5.4".into());
        let mut request = make_request();
        request.stream = streaming;
        request.requested_model = Some("gpt-5.6-sol".into());
        if streaming {
            let UpstreamStreamingResponse::Bytes(mut stream) = execute_stream(
                &client,
                Duration::from_secs(5),
                &payload,
                &request,
                "gpt-5.6-sol",
                None,
            )
            .await
            .unwrap() else {
                panic!("expected byte stream")
            };
            while let Some(chunk) = stream.next().await {
                chunk.unwrap();
            }
        } else {
            let response = execute(
                &client,
                Duration::from_secs(5),
                &payload,
                &request,
                "gpt-5.6-sol",
                None,
            )
            .await
            .unwrap();
            assert_eq!(response.text, "Paris");
        }
        let seen = state.seen.lock().unwrap();
        assert_eq!(
            seen.iter().map(|s| s.path.as_str()).collect::<Vec<_>>(),
            [
                "/backend-api/sentinel/chat-requirements",
                "/backend-api/conversation",
            ]
        );
        let body: Value = serde_json::from_str(&seen[1].body).unwrap();
        assert_eq!(body["model"], "gpt-5.6-sol");
        uuid::Uuid::parse_str(body["parent_message_id"].as_str().unwrap()).unwrap();
        assert_eq!(
            seen[1].headers.get("cookie").map(String::as_str),
            Some("session=fixture")
        );
    }
}

#[test]
fn text_compat_retains_explicit_and_cached_legacy_path_selection() {
    let mut payload = make_payload("https://chatgpt.com");
    assert_eq!(
        conversation_path(&payload, false),
        surface::CHATGPT_WEB_DEFAULT_CONVERSATION_PATH
    );
    assert_eq!(
        conversation_path(&payload, true),
        surface::CHATGPT_WEB_DEFAULT_F_CONVERSATION_PATH
    );
    for key in [
        "conversationPath",
        "chatgptConversationPath",
        "chatgptWebConversationPath",
    ] {
        payload
            .extra_body
            .as_mut()
            .unwrap()
            .insert(key.into(), json!("/backend-api/f/conversation"));
        assert_eq!(
            conversation_path(&payload, false),
            surface::CHATGPT_WEB_DEFAULT_F_CONVERSATION_PATH
        );
        payload.extra_body.as_mut().unwrap().remove(key);
    }
    payload.extra_body.as_mut().unwrap().insert(
        "chatgptWebConversationMode".into(),
        json!("legacy_f_conversation"),
    );
    assert_eq!(
        conversation_path(&payload, false),
        surface::CHATGPT_WEB_DEFAULT_F_CONVERSATION_PATH
    );
    payload
        .extra_body
        .as_mut()
        .unwrap()
        .remove("chatgptWebConversationMode");
    payload
        .extra_body
        .as_mut()
        .unwrap()
        .insert("chatgptWebDynamicBackendConversation".into(), json!(false));
    assert_eq!(
        conversation_path(&payload, false),
        surface::CHATGPT_WEB_DEFAULT_F_CONVERSATION_PATH
    );
}
