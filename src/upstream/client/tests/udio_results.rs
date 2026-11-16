//! Exercise result admission through the actual Udio owner, not only a decoder helper.
use super::*;
#[path = "udio_results/fixture.rs"]
mod fixture;
use fixture::*;

#[test]
fn udio_result_caller_headers_cannot_replace_or_create_account_session() {
    let mut payload = make_payload("udio_compatible", "https://www.udio.com");
    payload.api_key.clear();
    payload
        .headers
        .insert("cookie".into(), "saved=account-session".into());
    payload
        .headers
        .insert("user-agent".into(), "saved-account-UA".into());
    payload
        .headers
        .insert("origin".into(), "https://www.udio.com".into());
    let mut request = make_request(ProtocolFamily::OpenAi, EndpointKind::MusicGenerations);
    request.raw_body = json!({"prompt":"fixture"});
    let extra = HashMap::from([
        ("Cookie".into(), "foreign=caller-secret".into()),
        ("User-Agent".into(), "foreign-caller-UA".into()),
        ("Origin".into(), "https://caller.invalid".into()),
    ]);
    let prepare = |payload: &ProviderAccountPayload| {
        crate::upstream::udio_response_helpers::prepare_udio_execution_context(
            payload,
            &request,
            "fixture",
            Some(&extra),
            std::time::Duration::from_secs(1),
        )
    };
    let context = prepare(&payload).unwrap();
    assert_eq!(context.headers["cookie"], "saved=account-session");
    assert_eq!(context.headers["user-agent"], "saved-account-UA");
    assert_eq!(context.headers["origin"], "https://www.udio.com");
    payload.headers.remove("cookie");
    assert_eq!(
        prepare(&payload).unwrap_err().code.as_deref(),
        Some("missing_udio_browser_runtime")
    );
}

#[tokio::test]
async fn udio_result_red_rejects_partial_complete_snapshot() {
    assert_rejected(
        invoke(
            json!({"trackIds":["one","two"],
        "songs":[complete("one")],"completed":true}),
            true,
            EndpointKind::MusicGenerations,
        )
        .await,
    );
}
#[tokio::test]
async fn udio_result_red_rejects_foreign_song() {
    assert_rejected(
        invoke(
            json!({"trackIds":["one"],
        "songs":[complete("private-result")],"completed":true}),
            true,
            EndpointKind::MusicGenerations,
        )
        .await,
    );
}
#[tokio::test]
async fn udio_result_red_rejects_failed_finished_song() {
    let mut song = complete("one");
    song["state"] = json!("failed");
    assert_rejected(
        invoke(
            json!({"trackIds":["one"],
        "songs":[song],"completed":true}),
            true,
            EndpointKind::MusicGenerations,
        )
        .await,
    );
}
#[tokio::test]
async fn udio_result_red_default_wait_cannot_succeed_pending() {
    assert_rejected(
        invoke(
            json!({"trackIds":["one"],"songs":[],"completed":false}),
            true,
            EndpointKind::MusicGenerations,
        )
        .await,
    );
}

#[tokio::test]
async fn udio_result_no_wait_preserves_original_ids_including_missing_songs() {
    let result = invoke(json!({"trackIds":["one","two"],"songs":[{"id":"two","status":"queued"}],"completed":false}),
        false, EndpointKind::MusicGenerations).await.unwrap();
    assert_eq!(result["completed"], false);
    assert_eq!(result["data"][0]["id"], "one");
    assert_eq!(result["data"][0]["status"], "pending");
    assert_eq!(result["data"][1]["id"], "two");
    assert_eq!(result["data"][1]["status"], "queued");
}

#[tokio::test]
async fn udio_result_complete_music_restores_submission_order_without_fetching_assets() {
    let result = invoke(json!({"trackIds":["one","two"],"songs":[complete("two"),complete("one")],"completed":true}),
        true, EndpointKind::MusicGenerations).await.unwrap();
    assert_eq!(result["completed"], true);
    assert_eq!(result["data"][0]["id"], "one");
    assert_eq!(result["data"][1]["id"], "two");
    assert!(result.get("usage").is_none());
}
