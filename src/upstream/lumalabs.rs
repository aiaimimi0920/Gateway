use std::process::Stdio;

use rquest::Method;
use serde_json::Value;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::error::{classify_network_error, GatewayError};
use crate::protocol::canonical::CanonicalRelayRequest;
use crate::protocol::lumalabs;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::client::UpstreamClient;
use crate::upstream::lumalabs_response_helpers::{
    build_lumalabs_browser_executor_service_result, build_lumalabs_downloaded_image_response,
    build_lumalabs_non_image_generation_response, ensure_successful_lumalabs_media_fetch_status,
    parse_lumalabs_browser_worker_verified_output,
    parse_lumalabs_remote_browser_executor_signed_url, prepare_lumalabs_execution_context,
    resolve_lumalabs_image_generation_plan,
};
use crate::upstream::lumalabs_runtime_helpers::{
    build_lumalabs_browser_executor_payload_from_prepared,
    prepare_lumalabs_browser_executor_service_input, prepare_lumalabs_browser_worker_launch,
    PreparedLumalabsBrowserExecutorServiceInput,
};

impl UpstreamClient {
    pub(crate) async fn execute_lumalabs_browser_executor_service_invocation(
        &self,
        input: &Value,
    ) -> Result<Value, GatewayError> {
        let prepared = prepare_lumalabs_browser_executor_service_input(input)?;
        let signed_url = self
            .execute_lumalabs_browser_worker("lumalabs_compatible", &prepared)
            .await?;
        Ok(build_lumalabs_browser_executor_service_result(&signed_url))
    }

    pub(crate) async fn execute_lumalabs_media(
        &self,
        provider_account_id: &str,
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
        _extra_headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<Value, GatewayError> {
        let provider = "lumalabs_compatible";
        let prepared = prepare_lumalabs_execution_context(payload, req, model, self.timeout)?;
        let signed_url = if let Some(result) = self
            .execute_remote_browser_executor(
                "lumalabs",
                provider_account_id,
                req.endpoint_kind,
                build_lumalabs_browser_executor_payload_from_prepared(
                    &prepared.browser_input,
                    std::env::var("LUMALABS_BROWSER_EXECUTABLE_PATH").ok(),
                ),
            )
            .await?
        {
            parse_lumalabs_remote_browser_executor_signed_url(&result)?
        } else {
            self.execute_lumalabs_browser_worker(provider, &prepared.browser_input)
                .await?
        };

        match prepared.media_plan.operation {
            lumalabs::LumalabsMediaOperation::Image => {
                if let Some(response) = resolve_lumalabs_image_generation_plan(
                    req,
                    &prepared.media_plan.prompt,
                    &signed_url,
                )? {
                    return Ok(response);
                }

                let download_response = self
                    .http
                    .request(Method::GET, &signed_url)
                    .timeout(prepared.browser_input.timeout)
                    .send()
                    .await
                    .map_err(|e| classify_network_error(&e, Some(provider)))?;

                let download_status = download_response.status().as_u16();
                if !download_response.status().is_success() {
                    let body_text = download_response
                        .text()
                        .await
                        .unwrap_or_else(|_| String::from("<unreadable body>"));
                    ensure_successful_lumalabs_media_fetch_status(download_status, &body_text)?;
                    unreachable!("non-2xx LumaLabs image downloads should fail");
                }

                let response_headers = download_response.headers().clone();
                let bytes = download_response
                    .bytes()
                    .await
                    .map_err(|e| classify_network_error(&e, Some(provider)))?;

                Ok(build_lumalabs_downloaded_image_response(
                    &prepared.media_plan.prompt,
                    &signed_url,
                    &response_headers,
                    &bytes,
                ))
            }
            lumalabs::LumalabsMediaOperation::Video | lumalabs::LumalabsMediaOperation::Audio => {
                Ok(build_lumalabs_non_image_generation_response(
                    prepared.media_plan.operation,
                    model,
                    &prepared.media_plan.prompt,
                    &signed_url,
                ))
            }
        }
    }

    async fn execute_lumalabs_browser_worker(
        &self,
        provider: &str,
        prepared: &PreparedLumalabsBrowserExecutorServiceInput,
    ) -> Result<String, GatewayError> {
        let launch = prepare_lumalabs_browser_worker_launch(
            prepared,
            std::env::var("LUMALABS_BROWSER_EXECUTABLE_PATH").ok(),
            std::env::var("LUMALABS_BROWSER_NODE_BIN").ok(),
        )?;

        let mut child = Command::new(&launch.node_bin)
            .arg(&launch.script_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                lumalabs::browser_worker_spawn_failed_error(
                    launch.script_path.as_path(),
                    error.to_string().as_str(),
                )
            })?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(&launch.stdin_json).await.map_err(|error| {
                lumalabs::browser_worker_stdin_error(error.to_string().as_str())
            })?;
        }

        let output = child.wait_with_output().await.map_err(|error| {
            lumalabs::browser_worker_wait_failed_error(error.to_string().as_str())
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        parse_lumalabs_browser_worker_verified_output(&stdout, &stderr, provider)
    }
}
