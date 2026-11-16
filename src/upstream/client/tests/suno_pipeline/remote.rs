//! Remote browser metadata cannot bypass the local audio gate, even for no-wait callers.
use super::{
    peer::{payload, request},
    *,
};
use crate::upstream::client::UpstreamClient;
#[tokio::test]
async fn suno_music_remote_browser_result_uses_same_audio_gate() {
    for valid in [true, false] {
        let audio = if valid {
            Reply::audio()
        } else {
            Reply::status(200, "private HTML, not audio")
        };
        let server = Server::start(vec![
            Reply::json(
                json!({"ok":true,"result":{"clips":[clip("a","complete")],"completed":true,"wireModel":"chirp-auk-turbo"}}),
            ),
            audio,
        ])
        .await;
        let mut client = UpstreamClient::new(3);
        client.browser_executor_base_url = Some(server.origin.clone());
        client.browser_executor_bearer_token = Some("synthetic-executor".into());
        client.request_time_browser_policy =
            crate::upstream::request_time_browser_policy::RequestTimeBrowserPolicy::RemoteOnly;
        let result = crate::upstream::public_assets::within(
            &server.origin,
            client.execute_suno_browser_backed(
                "exact",
                &payload("https://studio-api.prod.suno.com"),
                &request(json!({"waitCompletion":false})),
                "chirp-v3-5",
                None,
            ),
        )
        .await;
        if valid {
            assert_eq!(result.unwrap()["completed"], true);
        } else {
            assert_eq!(result.unwrap_err().code.as_deref(), Some("suno_asset_type"));
        }
        let calls = server.calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(
            calls[0].path,
            "/v1/internal/browser-executor/suno-bound-v1/execute"
        );
        assert_eq!(
            calls[0].headers["authorization"],
            "Bearer synthetic-executor"
        );
        assert_eq!(calls[1].method, axum::http::Method::GET);
        for name in [
            "cookie",
            "authorization",
            "origin",
            "referer",
            "device-id",
            "browser-token",
        ] {
            assert!(!calls[1].headers.contains_key(name));
        }
        server.stop().await;
    }
}
