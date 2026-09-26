use super::*;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use std::sync::atomic::{AtomicBool, Ordering};

#[tokio::test]
async fn saturated_or_closed_capacity_rejects_before_preparing() {
    for closed in [false, true] {
        let slots = Arc::new(Semaphore::new(0));
        if closed {
            slots.close();
        }
        let prepared = AtomicBool::new(false);
        let error = run_with(slots, || {
            prepared.store(true, Ordering::SeqCst);
            || Ok(())
        })
        .await
        .expect_err("busy");
        assert!(!prepared.load(Ordering::SeqCst));
        assert_eq!(error.code.as_deref(), Some("chatgpt_web_solver_busy"));
        assert_eq!(error.kind, crate::error::ErrorKind::ServiceUnavailable);
        assert_eq!(error.provider_name.as_deref(), Some(PROVIDER));
    }
}

#[tokio::test(flavor = "current_thread")]
async fn solver_runs_off_executor() {
    let slots = Arc::new(Semaphore::new(1));
    let executor = std::thread::current().id();
    let worker = run_with(slots.clone(), || || Ok(std::thread::current().id()))
        .await
        .unwrap();
    assert_ne!(worker, executor, "solver must leave the Tokio core worker");
    assert_eq!(slots.available_permits(), 1);
}

#[tokio::test]
async fn solver_panic_returns_fixed_error_and_releases_capacity() {
    let slots = Arc::new(Semaphore::new(1));
    let error = run_with::<(), _>(slots.clone(), || || panic!("synthetic solver panic"))
        .await
        .expect_err("worker error");
    assert_eq!(
        error.code.as_deref(),
        Some("chatgpt_web_solver_worker_failed")
    );
    assert_eq!(
        error.message,
        "ChatGPT Web reverse proof solver worker failed."
    );
    assert_eq!(error.provider_name.as_deref(), Some(PROVIDER));
    assert_eq!(error.kind, crate::error::ErrorKind::ServerError);
    assert_eq!(slots.available_permits(), 1);
}

#[tokio::test]
async fn legacy_wrapper_preserves_browser_payload() {
    let token = legacy(&ChatGptWebBootstrap::default(), "solver-test-agent")
        .await
        .unwrap();
    let bytes = BASE64
        .decode(token.strip_prefix("gAAAAAC").unwrap())
        .unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(payload.as_array().unwrap().len(), 18);
    assert_eq!(payload[4], "solver-test-agent");
}

#[tokio::test]
async fn proof_wrapper_preserves_success_and_protocol_failure() {
    let bootstrap = ChatGptWebBootstrap::default();
    let token = proof(&bootstrap, "solver-test-agent", "seed", "ff")
        .await
        .unwrap();
    assert!(token.starts_with("gAAAAAB"));
    let error = proof(&bootstrap, "agent", "private-seed", "invalid")
        .await
        .expect_err("invalid difficulty");
    assert_eq!(
        error.code.as_deref(),
        Some("chatgpt_web_proof_token_failed")
    );
    assert_eq!(error.provider_name.as_deref(), Some(PROVIDER));
    assert!(!error.message.contains("private-seed"));
}

#[tokio::test]
async fn turnstile_wrapper_preserves_empty_key_then_legacy_fallback() {
    let program = br#"[[3,"turnstile-ok"]]"#;
    let legacy = "gAAAAAC-test-seed";
    let dx = BASE64.encode(
        program
            .iter()
            .enumerate()
            .map(|(i, b)| b ^ legacy.as_bytes()[i % legacy.len()])
            .collect::<Vec<_>>(),
    );
    assert_eq!(
        turnstile(&dx, legacy).await.unwrap(),
        Some(BASE64.encode("turnstile-ok"))
    );
    assert_eq!(
        turnstile(&BASE64.encode(program), legacy).await.unwrap(),
        Some(BASE64.encode("turnstile-ok"))
    );
    assert_eq!(turnstile("not base64", legacy).await.unwrap(), None);
}
