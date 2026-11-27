//! Imported remote accounts retain their lane when proxy discovery fails, and
//! unconfigured reserved models never spill into wildcard/ordinary upstreams.
use super::*;

#[test]
fn cursor_remote_http_startup_failure_and_drain_never_fall_back() {
    run(|| async {
        for missing in [
            "proxy_invalid",
            "proxy_required",
            "proxy_unsupported",
            "draining",
        ] {
            let mut state = app(true).await;
            let runtime = &mut Arc::get_mut(&mut state).unwrap().lifecycle.cursor_remote;
            if missing == "draining" {
                runtime.close();
            } else {
                *runtime = super::super::runtime::Runtime::unavailable(missing);
            }
            let token = key(&state, "responses", "cursor-remote").await;
            for store in [false, true] {
                let (status, body) = post(
                    &state,
                    &token,
                    json!({"model":"cursor-personal","input":"hello","store":store}),
                )
                .await;
                assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
                assert!(body.contains(&format!("cursor_remote_{missing}")), "{body}");
                assert!(!body.contains("cursor_acp"), "{body}");
            }
            let (status, body) = post(
                &state,
                &token,
                json!({"model":"cursor-personal","input":"hello","store":true,
                    "previous_response_id":"resp_gw_previous_fixture"}),
            )
            .await;
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
            assert!(body.contains("cursor_acp_execution_unavailable"), "{body}");
            let rows = tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    let rows = state
                        .local_runtime
                        .as_ref()
                        .unwrap()
                        .list_audits(&db::RequestAuditFilters::default())
                        .await
                        .unwrap();
                    if rows.len() == 3 && rows.iter().all(|row| row.completed_at.is_some()) {
                        break rows;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
            for row in rows {
                assert_eq!(row.route_trace.unwrap()["routeAttemptCount"], 0);
                assert!(row.provider_account_id.is_none());
                assert!(row.upstream_status.is_none());
            }
            state.lifecycle.cursor_remote.join().await;
            state.local_runtime.as_ref().unwrap().close().await;
        }
    });
}

#[test]
fn cursor_remote_http_ready_without_import_preserves_reserved_model_guard() {
    run(|| async {
        let state = app(true).await;
        assert!(state.lifecycle.cursor_remote.available());
        let token = key(&state, "responses", "cursor-remote").await;
        let mut document = state.route_config.snapshot().document().clone();
        document.providers.clear();
        state.route_config.replace_document(document).unwrap();
        for store in [false, true] {
            let (status, body) = post(
                &state,
                &token,
                json!({"model":"cursor-personal","input":"hello","store":store}),
            )
            .await;
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
            assert!(body.contains("cursor_acp_execution_unavailable"), "{body}");
        }
        state.lifecycle.cursor_remote.join().await;
        state.local_runtime.as_ref().unwrap().close().await;
    });
}

#[test]
fn cursor_remote_http_drain_after_route_keeps_frozen_lane_without_send() {
    run(|| async {
        let state = app(true).await;
        let token = key(&state, "responses", "cursor-remote").await;
        let request = crate::protocol::responses::normalize_responses(
            json!({"model":"cursor-personal","input":"hello","store":false}),
        )
        .unwrap();
        let mut ctx = crate::pipeline::PipelineContext::new(request, Some(token));
        crate::pipeline::stage_auth::run(&mut ctx, &state)
            .await
            .unwrap();
        crate::pipeline::stage_route::run(&mut ctx, &state)
            .await
            .unwrap();
        assert_eq!(ctx.candidates.len(), 1);
        assert_eq!(
            ctx.candidates[0].payload.auth_mode.as_deref(),
            Some("cursor_remote")
        );
        state.lifecycle.cursor_remote.close();
        let Err(error) = crate::pipeline::stage_send::run(&mut ctx, &state).await else {
            panic!("closed worker entered send");
        };
        assert_eq!(error.code.as_deref(), Some("cursor_remote_draining"));
        assert_eq!(ctx.route_attempt_count(), 0);
        assert!(ctx.observed_upstream_status.is_none());
        state.lifecycle.cursor_remote.join().await;
        state.local_runtime.as_ref().unwrap().close().await;
    });
}

#[test]
fn cursor_remote_forced_protocol_is_rejected_before_route_and_send() {
    run(|| async {
        let state = app(true).await;
        let token = key(&state, "responses", "cursor-remote").await;
        for after_route in [false, true] {
            let request = crate::protocol::responses::normalize_responses(
                json!({"model":"cursor-personal","input":"hello","store":false}),
            )
            .unwrap();
            let mut ctx = crate::pipeline::PipelineContext::new(request, Some(token.clone()));
            crate::pipeline::stage_auth::run(&mut ctx, &state)
                .await
                .unwrap();
            if after_route {
                crate::pipeline::stage_route::run(&mut ctx, &state)
                    .await
                    .unwrap();
                assert_eq!(ctx.candidates.len(), 1);
            }
            ctx.forced_upstream_protocol = Some("anthropic_messages".into());
            let error = if after_route {
                crate::pipeline::stage_send::run(&mut ctx, &state)
                    .await
                    .err()
            } else {
                crate::pipeline::stage_route::run(&mut ctx, &state)
                    .await
                    .err()
            }
            .expect("Cursor must not silently ignore a forced upstream protocol");
            assert_eq!(
                error.code.as_deref(),
                Some("debug_upstream_protocol_unavailable")
            );
            assert!(!error.retryable);
            if !after_route {
                assert!(ctx.candidates.is_empty());
            }
            assert_eq!(ctx.route_attempt_count(), 0);
            assert!(ctx.observed_upstream_status.is_none());
        }
        state.lifecycle.cursor_remote.join().await;
        state.local_runtime.as_ref().unwrap().close().await;
    });
}

#[test]
fn cursor_reserved_model_send_without_snapshot_never_uses_ordinary_candidates() {
    run(|| async {
        let state = app(false).await;
        let ordinary_doc: crate::routing::config::RouteConfigYaml = serde_json::from_value(json!({
            "providers":[{"id":"ordinary","base_url":"https://example.invalid","supported_models":["ordinary"]}]
        }))
        .unwrap();
        state.route_config.replace_document(ordinary_doc).unwrap();
        let candidates = state.route_config.resolve_candidates(Some("ordinary"));
        assert!(!candidates.is_empty());
        for model in ["cursor-personal", " CURSOR-PERSONAL "] {
            let req = crate::protocol::openai::normalize_chat_completions(
                json!({"model":model, "messages":[{"role":"user","content":"hello"}]}),
            )
            .unwrap();
            let mut ctx = crate::pipeline::PipelineContext::new(req, None);
            ctx.candidates = candidates.clone();
            assert!(ctx.cash_route_snapshot.is_none());
            let err = Box::pin(crate::pipeline::stage_send::run(&mut ctx, &state))
                .await
                .err()
                .expect("private model must not enter ordinary dispatch");
            assert_eq!(
                err.code.as_deref(),
                Some("cursor_acp_execution_unavailable")
            );
            assert!(!err.retryable);
            assert!(matches!(
                err.fallback_hint,
                crate::error::FallbackHint::Abort { .. }
            ));
            assert_eq!(ctx.route_attempt_count(), 0);
            assert!(ctx.observed_upstream_status.is_none());
        }
        state.lifecycle.cursor_remote.join().await;
        state.local_runtime.as_ref().unwrap().close().await;
    });
}

#[test]
fn cursor_reserved_model_http_without_account_never_reaches_wildcard_upstream() {
    run(|| async {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        let upstream = axum::Router::new().fallback(
            move |method: axum::http::Method, uri: axum::http::Uri| {
                let observed = observed.clone();
                async move {
                    observed.fetch_add(1, Ordering::SeqCst);
                    assert_eq!(method, axum::http::Method::POST);
                    assert_eq!(uri.path(), "/v1/chat/completions");
                    axum::Json(json!({
                        "id":"synthetic-chat","object":"chat.completion","created":0,
                        "model":"cursor-other",
                        "choices":[{"index":0,"message":{"role":"assistant","content":"ordinary ok"},
                            "finish_reason":"stop"}],
                        "usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}
                    }))
                }
            },
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            axum::serve(listener, upstream)
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
                })
                .await
                .unwrap();
        });

        let state = app(false).await;
        let wildcard_doc: crate::routing::config::RouteConfigYaml = serde_json::from_value(json!({
            "providers":[{"id":"ordinary","base_url":format!("http://{address}/v1"),
                "api_key":"synthetic-key","chat_completions_path":"/chat/completions","supported_models":[]}],
            "model_routes":[{"pattern":"cursor-*","provider_ids":["ordinary"]}]
        }))
        .unwrap();
        state.route_config.replace_document(wildcard_doc).unwrap();
        let token = state
            .local_runtime
            .as_ref()
            .unwrap()
            .save_access_key(
                None,
                db::UpsertAccessKeyInput {
                    owner_type: "user".into(),
                    owner_id: "fixture".into(),
                    resolved_project_id: "local".into(),
                    resolved_tenant_id: "local".into(),
                    key_kind: "normal".into(),
                    public_key_prefix: "sk-gw".into(),
                    display_name: "Wildcard guard test".into(),
                    expires_at: None,
                    bundle_ids: vec![],
                    metadata: Some(json!({
                        "scopes":["chat.completions","responses"],
                        "providerIds":["ordinary"],
                        "models":["cursor-personal","CURSOR-PERSONAL","cursor-other"]
                    })),
                },
            )
            .await
            .unwrap()
            .token
            .unwrap();

        for model in ["cursor-personal", "CURSOR-PERSONAL"] {
            for stream in [false, true] {
                let (status, body) = post(
                    &state,
                    &token,
                    json!({"model":model,"input":"hello","store":false,"stream":stream}),
                )
                .await;
                assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
                assert!(body.contains("cursor_acp_execution_unavailable"), "{body}");
                assert_eq!(calls.load(Ordering::SeqCst), 0);
            }
        }
        let (status, body) = post(
            &state,
            &token,
            json!({"model":"cursor-other","input":"hello","store":false}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert!(body.contains("ordinary ok"), "{body}");
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let _ = stop.send(());
        server.await.unwrap();
        state.lifecycle.cursor_remote.join().await;
        state.local_runtime.as_ref().unwrap().close().await;
    });
}
