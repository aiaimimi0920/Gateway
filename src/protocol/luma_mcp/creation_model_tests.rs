//! New models traverse normal admission and the synthetic supplier; no live generation.
use super::*;

fn packed(model: &str, kind: &str, extra: Value) -> Result<Value, super::super::McpError> {
    let mut options = json!({"model":model});
    options
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    let request: creation::Request = serde_json::from_value(
        json!({"kind":kind,"prompt":"test","budgetCredits":50,"options":options}),
    )
    .unwrap();
    request.validate()?;
    Ok(request.generate("task", "s1")?.1)
}

#[test]
fn luma_mcp_creation_profiles_match_captured_model_contracts() {
    let profiles: Value = serde_json::from_str(include_str!(
        "../../../manifests/luma-mcp-creation-models.json"
    ))
    .unwrap();
    let catalog: Value = serde_json::from_str(include_str!(
        "../../../docs/evidence/luma/gateway-luma-mcp-models-20261120/contracts/models.json"
    ))
    .unwrap();
    let descriptions: Vec<Value> = serde_json::from_str(include_str!("../../../docs/evidence/luma/gateway-luma-mcp-models-20261120/contracts/model-descriptions.json")).unwrap();
    assert_eq!(profiles.as_object().unwrap().len(), 18);
    for row in catalog["models"].as_array().unwrap() {
        let id = row["id"].as_str().unwrap();
        let profile = &profiles[id];
        assert_eq!(profile["kind"], row["modality"]);
        assert_eq!(profile["ratios"], row["capabilities"]["aspect_ratios"]);
        assert_eq!(
            profile["maxReferences"].as_u64(),
            Some(row["capabilities"]["max_references"].as_u64().unwrap_or(0))
        );
        let description = descriptions.iter().find(|d| d["model"] == id).unwrap();
        assert_eq!(
            profile["parameters"].as_object().unwrap().len(),
            description["parameters"]["properties"]
                .as_object()
                .unwrap()
                .len()
        );
        for (key, rules) in profile["parameters"].as_object().unwrap() {
            for (property, value) in rules.as_object().unwrap() {
                assert_eq!(
                    value,
                    &description["parameters"]["properties"][key][property]
                );
            }
        }
        if row["modality"] == "image" && id != "luma_uni_1" {
            assert_eq!(description["media_roles"], json!(["reference"]));
        }
    }
}

#[tokio::test]
async fn luma_mcp_creation_all_catalog_models_keep_selected_model_and_one_dispatch() {
    let catalog: Value = serde_json::from_str(include_str!(
        "../../../manifests/luma-mcp-creation-models.json"
    ))
    .unwrap();
    for (model, profile) in catalog.as_object().unwrap() {
        let kind = profile["kind"].as_str().unwrap();
        let f = Fixture::new(vec![
            opened(),
            started(),
            progress("completed", false, json!([])),
            usage(false),
        ])
        .await;
        let request = json!({"kind":kind,"prompt":"a blue circle","budgetCredits":50,"options":{"model":model}});
        let mut prepare = f.command("prepare", None);
        prepare["request"] = request.clone();
        let (status, response) = f.send(prepare).await;
        assert_eq!(status, 200, "{model}: {response}");
        assert!(f.peer.calls.lock().unwrap().is_empty());
        let id = response["task"]["id"].as_str().unwrap();
        let mut start = f.command("start", Some(id));
        start["request"] = request;
        let (status, response) = f.send(start.clone()).await;
        assert_eq!(status, 200, "{model}: {response}");
        assert_eq!(response["task"]["requestedModel"], *model);
        assert_eq!(f.send(start).await.0, 409);
        assert_eq!(f.act("read", Some(id)).await["state"], "completed");
        {
            let calls = f.peer.calls.lock().unwrap();
            let generated: Vec<_> = calls
                .iter()
                .filter(|(_, v)| {
                    matches!(
                        v["params"]["name"].as_str(),
                        Some("luma_generate_image" | "luma_generate_video")
                    )
                })
                .collect();
            assert_eq!(generated.len(), 1);
            assert_eq!(generated[0].1["params"]["arguments"]["model"], *model);
        }
        f.close().await;
    }
}

#[test]
fn luma_mcp_creation_models_pack_parameters_references_and_video_variants() {
    for (model, parameters) in [
        (
            "bytedance_seedream_5_flash",
            json!({"size":"1536x1024","outputFormat":"png"}),
        ),
        (
            "google_nano_banana_2",
            json!({"resolution":"512","thinkingLevel":"high","enableSearch":false,"temperature":0.0}),
        ),
        (
            "google_nano_banana_2_1",
            json!({"resolution":"4K","enableSearch":true}),
        ),
        (
            "ideogram_4_6",
            json!({"quality":"very_low","size":"2048x2048","background":"transparent"}),
        ),
        (
            "openai_gpt_image_2_5_flare",
            json!({"quality":"max","background":"transparent","outputFormat":"webp"}),
        ),
    ] {
        let body = packed(
            model,
            "image",
            json!({"parameters":parameters,"referenceUrls":["https://example.com/ref"]}),
        )
        .unwrap();
        assert_eq!(
            body["medias"],
            json!([{"url":"https://example.com/ref","role":"reference"}])
        );
        for (key, value) in parameters.as_object().unwrap() {
            let wire = match key.as_str() {
                "outputFormat" => "output_format",
                "thinkingLevel" => "thinking_level",
                "enableSearch" => "enable_search",
                _ => key,
            };
            assert_eq!(&body["parameters"][wire], value);
        }
        assert!(body.get("source").is_none());
    }
    assert_eq!(
        packed("luma_ray3_14_hdr", "video", json!({})).unwrap()["video"]["hdr"],
        true
    );
    assert_eq!(
        packed(
            "luma_ray3_14",
            "video",
            json!({"video":{"duration":"5s","loopVideo":true}})
        )
        .unwrap()["video"]["loop"],
        true
    );
    assert!(packed("luma_ray3_14", "video", json!({"video":{"duration":"10s"}})).is_ok());
    assert!(packed(
        "ideogram_vector",
        "image",
        json!({"aspectRatio":"16x9","parameters":{"quality":"high"}})
    )
    .is_ok());
}

#[test]
fn luma_mcp_creation_models_reject_ignored_or_changed_parameters() {
    for (model, kind, options) in [
        (
            "bytedance_seedream",
            "image",
            json!({"referenceUrls":vec!["https://example.com/ref";6]}),
        ),
        (
            "bytedance_seedream_5_flash",
            "image",
            json!({"aspectRatio":"16:9"}),
        ),
        (
            "bytedance_seedream_5_flash",
            "image",
            json!({"parameters":{"size":"100x100"}}),
        ),
        (
            "bytedance_seedream_5_flash",
            "image",
            json!({"parameters":{"size":"999999999999999999x1000"}}),
        ),
        (
            "bytedance_seedream_5_pro",
            "image",
            json!({"parameters":{"resolution":"4K"}}),
        ),
        (
            "google_nano_banana_2_lite",
            "image",
            json!({"parameters":{"resolution":"2K"}}),
        ),
        (
            "google_nano_banana_2_1",
            "image",
            json!({"parameters":{"temperature":1.0}}),
        ),
        (
            "google_nano_banana_2",
            "image",
            json!({"parameters":{"temperature":2.1}}),
        ),
        (
            "ideogram_4_6",
            "image",
            json!({"parameters":{"quality":"very_low","size":"2304x4096"}}),
        ),
        (
            "ideogram_4_5",
            "image",
            json!({"parameters":{"magicPrompt":"on"},"referenceUrls":["https://example.com/ref"]}),
        ),
        (
            "openai_gpt_image_1_5",
            "image",
            json!({"aspectRatio":"16:9"}),
        ),
        (
            "openai_gpt_image_2_5_sunburst",
            "image",
            json!({"parameters":{"background":"transparent","outputFormat":"jpeg"}}),
        ),
        (
            "luma_ray3_14",
            "video",
            json!({"video":{"duration":"10s","startFrameUrl":"https://example.com/ref"}}),
        ),
        (
            "luma_ray3_14",
            "video",
            json!({"video":{"duration":"10s","loopVideo":true}}),
        ),
        ("luma_ray3_14", "video", json!({"video":{"hdr":true}})),
        (
            "luma_ray3_14_hdr",
            "video",
            json!({"video":{"duration":"10s"}}),
        ),
        ("luma_ray3_14_hdr", "video", json!({"video":{"hdr":false}})),
        (
            "luma_ray3_14_hdr",
            "video",
            json!({"video":{"resolution":"540p"}}),
        ),
        ("luma_ray3_2", "video", json!({"video":{"loopVideo":true}})),
    ] {
        assert!(
            packed(model, kind, options.clone()).is_err(),
            "{model}: {options}"
        );
    }
}

#[test]
fn luma_mcp_creation_models_preserve_prepared_parameter_serialization() {
    let request: creation::Request =
        serde_json::from_value(json!({"kind":"image","prompt":"test","budgetCredits":50,
        "options":{"model":"openai_gpt_image_2","parameters":{"quality":"high"}}}))
        .unwrap();
    let value = serde_json::to_value(request).unwrap();
    assert_eq!(
        value["options"]["parameters"],
        json!({"resolution":null,"quality":"high","outputFormat":null,"background":null})
    );
    let request: creation::Request = serde_json::from_value(json!({"kind":"video","prompt":"test","budgetCredits":50,"options":{"video":{"duration":"5s"}}})).unwrap();
    assert!(serde_json::to_value(request).unwrap()["options"]["video"]
        .get("loopVideo")
        .is_none());
}
