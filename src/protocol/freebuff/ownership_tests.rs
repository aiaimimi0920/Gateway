//! Business-owned wire fields and reauthorization namespaces; no real credentials or upstreams.
use super::tests::{make_payload, make_request};
use super::*;
mod peer;

#[test]
fn business_ownership_reauthorization_or_target_change_does_not_reuse_cached_state() {
    let original = make_payload();
    let config = FreeBuffRuntimeConfig::from_payload(&original, "z-ai/glm-5.1").unwrap();
    for field in ["token", "origin", "credential"] {
        let mut changed = original.clone();
        match field {
            "token" => changed.api_key = "SYNTHETIC_NEW_AUTH_TOKEN".into(),
            "origin" => changed.base_url = "https://other.example.test".into(),
            _ => changed.credential_id = Some("another-credential".into()),
        }
        let next = FreeBuffRuntimeConfig::from_payload(&changed, "z-ai/glm-5.1").unwrap();
        assert_ne!(
            next.session_bucket_key, config.session_bucket_key,
            "{field}"
        );
        assert_ne!(next.bucket_key, config.bucket_key, "{field}");
        assert!(!next.session_bucket_key.contains(&changed.api_key));
    }
    let mut equivalent = original.clone();
    equivalent.base_url = " https://www.codebuff.com/ ".into();
    equivalent.api_key = format!(" {} ", original.api_key);
    equivalent.credential_id = Some(" cred-1 ".into());
    assert_eq!(
        credential_subject_id(&equivalent),
        credential_subject_id(&original)
    );
}

#[test]
fn business_ownership_caller_cannot_leave_platform_session_claims() {
    let mut req = make_request();
    req.extra.insert("codebuff_metadata".into(), json!({
        "custom":"keep", "run_id":"attacker-run", "cost_mode":"paid", "client_id":"attacker-client",
        "freebuff_instance_id":"attacker-instance", "freebuff_multi_session":"1", "surface":"cli"
    }));
    let body = build_chat_request_body(&req, "model", false, "owned-run", "free", None);
    let metadata = &body["codebuff_metadata"];
    assert_eq!(metadata["custom"], "keep");
    assert_eq!(metadata["run_id"], "owned-run");
    assert_eq!(metadata["cost_mode"], "free");
    assert!(metadata.get("freebuff_instance_id").is_none());
    assert!(metadata.get("freebuff_multi_session").is_none());
    assert!(metadata.get("surface").is_none());
    assert_ne!(metadata["client_id"], "attacker-client");
    let plan = build_request_plan(&make_payload(), &req, "model", false).unwrap();
    assert_eq!(
        plan.body.unwrap()["codebuff_metadata"],
        json!({"custom":"keep"})
    );
}

#[test]
fn business_ownership_non_object_metadata_cannot_disable_run_fields() {
    for value in [
        json!(null),
        json!(false),
        json!(12),
        json!("opaque"),
        json!([]),
    ] {
        let mut req = make_request();
        req.extra.insert("codebuff_metadata".into(), value);
        let body = build_chat_request_body(
            &req,
            "model",
            true,
            "owned-run",
            "free",
            Some("owned-instance"),
        );
        assert_eq!(body["codebuff_metadata"]["run_id"], "owned-run");
        assert_eq!(
            body["codebuff_metadata"]["freebuff_instance_id"],
            "owned-instance"
        );
    }
}

#[test]
fn business_ownership_runtime_controls_do_not_escape_in_caller_body() {
    let mut req = make_request();
    for name in [
        "freebuffAgentId",
        "FREEBUFF_MODEL_AGENT_MAP",
        "freebuff-session-path",
        "freebuffCostMode",
    ] {
        req.extra.insert(name.into(), json!("not-a-wire-field"));
    }
    req.extra.insert("temperature".into(), json!(0.3));
    for body in [
        build_chat_request_body(&req, "model", false, "run", "free", None),
        build_request_plan(&make_payload(), &req, "model", false)
            .unwrap()
            .body
            .unwrap(),
    ] {
        assert!(body
            .as_object()
            .unwrap()
            .keys()
            .all(|key| !is_reserved_payload_extra_key(key)));
        assert_eq!(body["temperature"], 0.3);
    }
}

#[tokio::test]
async fn business_ownership_tcp_same_account_reauthorization_gets_its_own_session_and_run() {
    let peer = peer::Peer::start(false).await;
    let owner = Arc::new(RunRuntime::default());
    let mut payload = peer.payload();
    let client = peer.client();
    let mut req = make_request();
    req.extra.insert(
        "codebuff_metadata".into(),
        json!({"custom":"keep","freebuff_instance_id":"caller-instance"}),
    );
    for token in ["synthetic-old", "synthetic-new"] {
        payload.api_key = token.into();
        execute(&owner, &client, &payload, &req, "z-ai/glm-5.1", None)
            .await
            .unwrap();
    }
    owner.shutdown(Duration::from_secs(3)).await.unwrap();
    let hits = peer.close().await;
    for suffix in ["old", "new"] {
        let auth = format!("Bearer synthetic-{suffix}");
        assert_eq!(
            hits.iter()
                .filter(|hit| hit.auth == auth && hit.path.ends_with("/session"))
                .count(),
            1
        );
        let own = hits
            .iter()
            .filter(|hit| hit.auth == auth)
            .collect::<Vec<_>>();
        assert_eq!(own.len(), 4, "session, START, chat, FINISH for {suffix}");
        let chat = own
            .iter()
            .find(|hit| hit.path.ends_with("/completions"))
            .unwrap();
        assert_eq!(
            chat.body["codebuff_metadata"]["run_id"],
            format!("run-{suffix}")
        );
        assert_eq!(
            chat.body["codebuff_metadata"]["freebuff_instance_id"],
            format!("instance-{suffix}")
        );
        assert_eq!(chat.body["codebuff_metadata"]["custom"], "keep");
        assert_eq!(
            own.iter()
                .filter(|hit| hit.body["action"] == "FINISH")
                .count(),
            1
        );
    }
}

#[tokio::test]
async fn business_ownership_tcp_disabled_session_does_not_forward_caller_instance() {
    let peer = peer::Peer::start(true).await;
    let owner = Arc::new(RunRuntime::default());
    let mut req = make_request();
    req.extra.insert(
        "codebuff_metadata".into(),
        json!({"freebuff_instance_id":"caller-instance","freebuff_multi_session":"1"}),
    );
    execute(
        &owner,
        &peer.client(),
        &peer.payload(),
        &req,
        "z-ai/glm-5.1",
        None,
    )
    .await
    .unwrap();
    owner.shutdown(Duration::from_secs(3)).await.unwrap();
    let hits = peer.close().await;
    let chat = hits
        .iter()
        .find(|hit| hit.path.ends_with("/completions"))
        .unwrap();
    assert!(chat.body["codebuff_metadata"]
        .get("freebuff_instance_id")
        .is_none());
    assert!(chat.body["codebuff_metadata"]
        .get("freebuff_multi_session")
        .is_none());
}
