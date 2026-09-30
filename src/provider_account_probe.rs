use super::{
    build_probe_http_client, db, freebuff, probe_known_http_payload, send_probe_request, AppState,
    GatewayError, Method, ProbeExpectation, ProbePolicy,
};

pub async fn probe_provider_account_payload(
    state: &AppState,
    provider_account: &db::GatewayProviderAccountView,
) -> Result<(), GatewayError> {
    let payload = crate::routing::candidate::deserialize_provider_payload(
        &provider_account.payload,
        Some(&provider_account.adapter),
    )
    .map_err(|error| {
        GatewayError::server_error(format!(
            "deserialize provider payload for probe {}: {error}",
            provider_account.id
        ))
    })?;
    if crate::protocol::chatgpt::official_api::is_chatgpt_codex_backend_payload(&payload) {
        return Ok(());
    }

    let client = build_probe_http_client(state)?;
    if let Some(result) = probe_known_http_payload(&client, &payload, ProbePolicy::Legacy).await {
        return result;
    }

    match payload.canonical_adapter() {
        "freebuff_compatible" => {
            freebuff::probe_payload(&state.upstream_client.freebuff, &client, &payload).await
        }
        "udio_compatible" => {
            let base_url = payload.base_url.trim_end_matches('/');
            send_probe_request(
                &client,
                Method::GET,
                format!("{base_url}/api/users/current"),
                &payload,
                ProbeExpectation::HttpOk,
            )
            .await
        }
        _ => Ok(()),
    }
}
