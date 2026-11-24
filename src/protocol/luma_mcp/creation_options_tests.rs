//! Exercise option binding through the real management route, not just the mapper.
use super::*;

fn request(kind: &str, options: Value) -> Value {
    json!({"kind":kind,"prompt":"change the sky; keep the subject","budgetCredits":40,"options":options})
}
fn packed(kind: &str, options: Value) -> Result<Value, super::super::McpError> {
    let input: creation::Request = serde_json::from_value(request(kind, options)).unwrap();
    input.validate()?;
    Ok(input.generate("task", "s1")?.1)
}

#[tokio::test]
async fn luma_mcp_creation_options_bind_native_edits_and_third_party_parameters() {
    let cases = [
        (
            "image",
            json!({"model":"luma_uni_1","operation":"edit","sourceUrl":"https://cdn.example.com/source.png",
            "referenceUrls":["https://cdn.example.com/ref.png"],"style":"auto","outputFormat":"png"}),
            json!({"model":"luma_uni_1","type":"image_edit","source":{"url":"https://cdn.example.com/source.png"},
                "image_ref":[{"url":"https://cdn.example.com/ref.png"}],"style":"auto","output_format":"png"}),
        ),
        (
            "image",
            json!({"model":"openai_gpt_image_2","aspectRatio":"3:2",
            "referenceUrls":["https://cdn.example.com/ref.png"],"parameters":{"resolution":"4K","quality":"high","outputFormat":"webp","background":"opaque"}}),
            json!({"model":"openai_gpt_image_2","type":"image","aspect_ratio":"3:2",
                "medias":[{"url":"https://cdn.example.com/ref.png","role":"reference"}],
                "parameters":{"resolution":"4K","quality":"high","output_format":"webp","background":"opaque"}}),
        ),
        (
            "video",
            json!({"model":"luma_ray3_2","operation":"edit","sourceUrl":"https://cdn.example.com/source.webm",
            "sourceMediaType":"video/webm","video":{"resolution":"720p","hdr":true,"edit":{"strength":"adhere_2","autoControls":false}}}),
            json!({"model":"luma_ray3_2","type":"video_edit","source":{"url":"https://cdn.example.com/source.webm","media_type":"video/webm"},
                "video":{"resolution":"720p","hdr":true,"edit":{"strength":"adhere_2","auto_controls":false}}}),
        ),
        (
            "video",
            json!({"model":"luma_ray3_2","operation":"reframe","sourceUrl":"https://cdn.example.com/source.mp4",
            "sourceMediaType":"video/mp4","aspectRatio":"16:9","video":{"duration":"10s","sourcePosition":{"x":-0.5,"y":0.5,"width":1.5,"height":0.5}}}),
            json!({"model":"luma_ray3_2","type":"video_reframe","source":{"url":"https://cdn.example.com/source.mp4","media_type":"video/mp4"},
                "aspect_ratio":"16:9","video":{"duration":"10s","source_position":{"x_norm":-0.5,"y_norm":0.5,"w_norm":1.5,"h_norm":0.5}}}),
        ),
    ];
    for (kind, options, expected) in cases {
        let f = Fixture::new(vec![opened(), started(), progress("completed", false,
            json!([{"asset_id":"a1","type":kind}])),
            json!({"assets":[{"asset_id":"a1","type":kind,"has_image":true,"state":"ready","url":"https://cdn.example.com/result"}]}),usage(false)]).await;
        let mut prepare = f.command("prepare", None);
        prepare["request"] = request(kind, options.clone());
        let (status, saved) = f.send(prepare).await;
        assert_eq!(status, 200, "{saved}");
        let id = saved["task"]["id"].as_str().unwrap();
        assert!(f.peer.calls.lock().unwrap().is_empty());
        let mut start = f.command("start", Some(id));
        start["request"] = request(kind, options);
        let mut changed = start.clone();
        changed["request"]["options"]["sourceUrl"] = json!("https://cdn.example.com/different");
        assert_eq!(f.send(changed).await.0, 400);
        assert!(f.peer.calls.lock().unwrap().is_empty());
        let (status, started) = f.send(start.clone()).await;
        assert_eq!(status, 200, "{started}");
        assert_eq!(started["task"]["requestedModel"], expected["model"]);
        assert_eq!(started["task"]["operation"], expected["type"]);
        assert_eq!(f.send(start).await.0, 409);
        let done = f.act("read", Some(id)).await;
        assert_eq!(done["state"], "completed");
        assert_eq!(done["assets"][0]["kind"], kind);
        {
            let calls = f.peer.calls.lock().unwrap();
            let generation: Vec<_> = calls
                .iter()
                .filter(|(_, v)| {
                    matches!(
                        v["params"]["name"].as_str(),
                        Some("luma_generate_image" | "luma_generate_video")
                    )
                })
                .collect();
            assert_eq!(generation.len(), 1);
            let mut actual = generation[0].1["params"]["arguments"].clone();
            for key in ["session_id", "idempotency_key", "user_id", "prompt"] {
                actual.as_object_mut().unwrap().remove(key);
            }
            assert_eq!(actual, expected);
        }
        let row: String = sqlx::query_scalar("SELECT payload FROM request_audits WHERE id = ?")
            .bind(id)
            .fetch_one(&f.state.local_runtime.as_ref().unwrap().pool)
            .await
            .unwrap();
        let audit: Value = serde_json::from_str(&row).unwrap();
        assert_eq!(audit["requestedModel"], expected["model"]);
        assert!(audit["resolvedModel"].is_null());
        assert!(!row.contains("cdn.example.com/source"));
        assert!(!row.contains("change the sky"));
        f.close().await;
    }
}

#[test]
fn luma_mcp_creation_options_reject_incompatible_inputs_and_keep_old_digest() {
    let old = json!({"kind":"image","prompt":"circle","budgetCredits":20});
    let parsed: creation::Request = serde_json::from_value(old.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), old);
    for (kind, options) in [
        ("image", json!({"operation":"edit"})),
        ("image", json!({"model":"not-in-contract"})),
        ("image", json!({"model":"luma_ray3_2"})),
        ("image", json!({"sourceUrl":"https://example.com/a"})),
        (
            "image",
            json!({"operation":"edit","sourceUrl":"https://example.com/a","aspectRatio":"1:1"}),
        ),
        (
            "image",
            json!({"model":"openai_gpt_image_2","style":"manga"}),
        ),
        (
            "image",
            json!({"model":"openai_gpt_image_2","operation":"edit","sourceUrl":"https://example.com/a"}),
        ),
        ("image", json!({"parameters":{"quality":"high"}})),
        (
            "image",
            json!({"model":"openai_gpt_image_2","parameters":{"quality":"ultra"}}),
        ),
        (
            "image",
            json!({"model":"openai_gpt_image_2","parameters":{"outputFormat":"jpeg","background":"transparent"}}),
        ),
        (
            "image",
            json!({"referenceUrls":vec!["https://example.com/a";10]}),
        ),
        (
            "image",
            json!({"model":"openai_gpt_image_2","referenceUrls":vec!["https://example.com/a";17]}),
        ),
        (
            "video",
            json!({"operation":"reframe","sourceUrl":"https://example.com/a","sourceMediaType":"video/mp4"}),
        ),
        (
            "video",
            json!({"operation":"edit","sourceUrl":"https://example.com/a"}),
        ),
        (
            "video",
            json!({"operation":"edit","sourceUrl":"https://example.com/a","sourceMediaType":"image/png"}),
        ),
        ("video", json!({"video":{"hdr":true,"resolution":"360p"}})),
        ("video", json!({"video":{"edit":{"strength":"flex_1"}}})),
        (
            "video",
            json!({"operation":"edit","sourceUrl":"https://example.com/a","sourceMediaType":"video/mp4","video":{"duration":"5s"}}),
        ),
    ] {
        assert!(packed(kind, options.clone()).is_err(), "{kind}: {options}");
    }
    for url in [
        "http://example.com/a",
        "https://user:password@example.com/a",
        "https://127.0.0.1/a",
        "https://[::1]/a",
        "https://example.com/a#part",
    ] {
        assert!(packed("image", json!({"referenceUrls":[url]})).is_err());
    }
    for extra in [
        json!({"parameters":{"seed":42}}),
        json!({"video":{"loop":true}}),
        json!({"session_id":"foreign"}),
    ] {
        assert!(serde_json::from_value::<creation::Request>(request("video", extra)).is_err());
    }
}

#[test]
fn luma_mcp_creation_keyframes_follow_the_ray32_timeline() {
    let frames = json!({"duration":"10s","keyframes":[{"index":0,"url":"https://example.com/first"},{"index":240,"url":"https://example.com/last"}]});
    let body = packed("video", json!({"video":frames})).unwrap();
    assert_eq!(body["video"]["keyframe_indexes"], json!([0, 240]));
    assert_eq!(
        body["video"]["keyframes"][1],
        json!({"url":"https://example.com/last"})
    );
    for invalid in [
        json!({"keyframes":[{"index":121,"url":"https://example.com/a"}]}),
        json!({"duration":"10s","keyframes":[{"index":241,"url":"https://example.com/a"}]}),
        json!({"keyframes":[{"index":0,"url":"https://example.com/a"},{"index":0,"url":"https://example.com/b"}]}),
        json!({"startFrameUrl":"https://example.com/a","keyframes":[{"index":0,"url":"https://example.com/b"}]}),
    ] {
        assert!(packed("video", json!({"video":invalid})).is_err());
    }
}

#[tokio::test]
async fn luma_mcp_invalid_options_never_open_a_session_or_create_an_audit() {
    let f = Fixture::new(vec![]).await;
    let before: i64 = sqlx::query_scalar("SELECT count(*) FROM request_audits")
        .fetch_one(&f.state.local_runtime.as_ref().unwrap().pool)
        .await
        .unwrap();
    for options in [
        json!({"model":"luma_uni_1","operation":"edit"}),
        json!({"model":"openai_gpt_image_2","parameters":{"quality":"ultra"}}),
    ] {
        let mut input = f.command("prepare", None);
        input["request"] = request("image", options);
        assert_eq!(f.send(input).await.0, 400);
    }
    let after: i64 = sqlx::query_scalar("SELECT count(*) FROM request_audits")
        .fetch_one(&f.state.local_runtime.as_ref().unwrap().pool)
        .await
        .unwrap();
    assert_eq!(before, after);
    assert!(f.peer.calls.lock().unwrap().is_empty());
    f.close().await;
}
