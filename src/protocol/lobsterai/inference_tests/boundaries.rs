//! Unsupported model contracts and request features fail before network IO.
use super::*;
#[tokio::test]
async fn lobsterai_declared_model_and_lossless_input_gate() {
    let peer = peer::Peer::new("chat", false, false, 200).await;
    let fixture = Box::pin(Fixture::new(&peer)).await;
    let snapshot = fixture.state.route_config.snapshot();
    let (_, credential) = snapshot
        .oauth_refresh_target("lobsterai-public", "exact")
        .unwrap();
    let payload = credential.payload.clone();
    let req = crate::protocol::openai::normalize_chat_completions(body("chat/completions", false))
        .unwrap();
    let plan = super::super::runtime::plan(&payload, &req, UPSTREAM_MODEL, false).unwrap();
    assert_eq!(plan.body.as_ref().unwrap()["model"], UPSTREAM_MODEL);
    assert!(!plan
        .body
        .as_ref()
        .unwrap()
        .to_string()
        .contains("gateway_lobsterai"));
    assert!(super::super::runtime::plan(&payload, &req, "unknown", false).is_err());
    for patch in [
        json!({"apiFormat":"gemini"}),
        json!({"runtimeProfile":"moonshot-kimi-k3"}),
        json!({"runtimeProfile":"unknown"}),
    ] {
        let mut p = payload.clone();
        let metadata = p.extra_body.as_mut().unwrap().values_mut().next().unwrap();
        let model = &mut metadata["catalog"]["models"][0];
        for (key, value) in patch.as_object().unwrap() {
            model[key] = value.clone();
        }
        let id = model["modelId"].as_str().unwrap().to_owned();
        assert!(super::super::runtime::plan(&p, &req, &id, false).is_err());
    }
    for (key, value) in [
        ("reasoning_effort", json!("high")),
        ("n", json!(2)),
        ("store", json!(true)),
        ("audio", json!({})),
    ] {
        let mut raw = body("chat/completions", false);
        raw[key] = value;
        let req = crate::protocol::openai::normalize_chat_completions(raw).unwrap();
        assert!(super::super::runtime::plan(&payload, &req, UPSTREAM_MODEL, false).is_err());
    }
    assert!(peer.calls.lock().unwrap().is_empty());
    fixture.finish().await;
    peer.stop().await;
}
