//! One private session owner surrounds local/remote dispatch, including business failures.
use super::{browser_worker, PreparedLumalabsBrowserExecutorServiceInput};
use crate::{
    error::GatewayError, luma_web_session::error, protocol::canonical::EndpointKind,
    routing::candidate::ProviderAccountPayload, upstream::client::UpstreamClient,
};
use serde_json::Value;

impl UpstreamClient {
    pub(super) async fn dispatch_luma_session(
        &self,
        provider: &str,
        payload: &ProviderAccountPayload,
        endpoint: EndpointKind,
        mut prepared: PreparedLumalabsBrowserExecutorServiceInput,
        session_only: bool,
    ) -> Result<Option<Value>, GatewayError> {
        self.validate_luma_browser_dispatch()?;
        let store = self.luma_web_sessions.get();
        let attempt = if let Some(store) = store {
            match store.begin(provider, payload, session_only).await? {
                Some(attempt) => Some(attempt),
                None => return Ok(None),
            }
        } else {
            None
        };
        if let Some(attempt) = &attempt {
            prepared.session_token = attempt.token().into();
        }
        let mut input = super::build_lumalabs_browser_executor_payload_from_prepared(
            &prepared,
            std::env::var("LUMALABS_BROWSER_EXECUTABLE_PATH").ok(),
        );
        if let Some(attempt) = &attempt {
            input["sessionBinding"] = attempt.binding().into();
        }
        input["sessionOnly"] = session_only.into();
        let mut value = if let Some(value) = self
            .execute_remote_browser_executor("lumalabs", provider, endpoint, input.clone())
            .await?
        {
            value
        } else {
            match self.execute_luma_worker_value(&prepared, &input).await {
                Ok(value) => value,
                Err(error) => {
                    // These exact errors occur before a worker can dispatch any upstream request.
                    if matches!(
                        error.code.as_deref(),
                        Some(
                            "lumalabs_browser_worker_spawn_failed"
                                | "lumalabs_browser_worker_input_too_large"
                        )
                    ) {
                        if let (Some(store), Some(attempt)) = (store, attempt.as_ref()) {
                            store.release_unstarted(attempt).await?;
                        }
                    }
                    return Err(error);
                }
            }
        };
        // Remove before parsing any business result. Cookie bytes never enter media or error JSON.
        let receipt = value.as_object_mut().and_then(|v| v.remove("session"));
        if let (Some(store), Some(attempt)) = (store, attempt.as_ref()) {
            store
                .finish(
                    attempt,
                    receipt.ok_or_else(|| error("lumalabs_session_receipt_missing"))?,
                )
                .await?;
        }
        Ok(Some(value))
    }

    pub(super) async fn execute_luma_worker_value(
        &self,
        prepared: &PreparedLumalabsBrowserExecutorServiceInput,
        input: &Value,
    ) -> Result<Value, GatewayError> {
        let mut launch = super::prepare_lumalabs_browser_worker_launch(
            prepared,
            std::env::var("LUMALABS_BROWSER_EXECUTABLE_PATH").ok(),
            std::env::var("LUMALABS_BROWSER_NODE_BIN").ok(),
        )?;
        let mut stdin: Value =
            serde_json::from_slice(&launch.stdin_json).expect("serialized worker input");
        for field in ["sessionBinding", "sessionOnly"] {
            if let Some(value) = input.get(field) {
                stdin[field] = value.clone();
            }
        }
        launch.stdin_json = serde_json::to_vec(&stdin).expect("serializable worker input");
        browser_worker::run_value(launch, prepared.timeout, "lumalabs_compatible").await
    }

    pub(crate) async fn maintain_luma_web_session(
        &self,
        provider: &str,
        payload: &ProviderAccountPayload,
    ) -> Result<(), GatewayError> {
        let runtime = crate::protocol::lumalabs::runtime_from_payload(payload)?;
        let prepared = PreparedLumalabsBrowserExecutorServiceInput {
            base_url: payload.base_url.trim_end_matches('/').into(),
            realm_id: runtime.realm_id,
            media_operation: None,
            artifact_field: "image".into(),
            session_token: payload.api_key.clone(),
            user_agent: payload
                .headers
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case("user-agent"))
                .map(|(_, v)| v.clone()),
            action_body: serde_json::json!({}),
            auto_discover_action_type: false,
            locale: crate::upstream::lumalabs_runtime_helpers::lumalabs_locale(payload).into(),
            timeout: std::time::Duration::from_secs(30),
        };
        if let Some(value) = self
            .dispatch_luma_session(
                provider,
                payload,
                EndpointKind::ImagesGenerations,
                prepared,
                true,
            )
            .await?
        {
            if value.get("ok").and_then(Value::as_bool) != Some(true) {
                return Err(error(
                    if value.pointer("/error/status").and_then(Value::as_u64) == Some(401) {
                        "lumalabs_session_login_required"
                    } else {
                        "lumalabs_session_check_failed"
                    },
                ));
            }
        }
        Ok(())
    }
}
