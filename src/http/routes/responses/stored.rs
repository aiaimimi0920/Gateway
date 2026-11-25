//! Only canonical HTTP Responses exposes SAP's opted-in stored lifecycle.
use super::*;
use crate::{
    http::request_headers::build_public_auth_request,
    pipeline::stored_responses::{self, Request},
    protocol::sap_aicore::responses_lifecycle::Operation,
};
use axum::{
    extract::{OriginalUri, Path},
    http::Method,
};

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StoredQuery {
    stream: Option<bool>,
    starting_after: Option<i32>,
}

pub(crate) async fn handle_public_create(
    state: State<Arc<AppState>>,
    token: OptionalBearerToken,
    query: Query<HashMap<String, String>>,
    original: OriginalUri,
    method: Method,
    mut headers: HeaderMap,
    body: NativeJson,
) -> Result<Response, GatewayError> {
    if body.value.get("store").and_then(serde_json::Value::as_bool) != Some(true) {
        return Box::pin(super::handle_responses(state, token, query, headers, body)).await;
    }
    crate::http::native_session::normalize_headers(&mut headers)?;
    let auth = build_public_auth_request(
        &state.config,
        token.0.as_deref(),
        &headers,
        original.0.path(),
        method.as_str(),
    );
    let mut ctx = prepare_context(body.value, token.0, query.0, &headers, &state.0)?;
    ctx.native_wire = Some(Arc::new(body.wire));
    super::respond(Box::pin(stored_responses::create(ctx, auth, state.0, headers)).await?)
}

pub(crate) async fn handle_stored_operation(
    State(state): State<Arc<AppState>>,
    OptionalBearerToken(token): OptionalBearerToken,
    Path(id): Path<String>,
    Query(query): Query<StoredQuery>,
    OriginalUri(original): OriginalUri,
    method: Method,
    mut headers: HeaderMap,
) -> Result<Response, GatewayError> {
    if query.starting_after.is_some_and(|n| n < 0)
        || (query.starting_after.is_some() && query.stream != Some(true))
        || (method != Method::GET && (query.stream.is_some() || query.starting_after.is_some()))
    {
        return Err(GatewayError::bad_request(
            "Invalid stored Responses stream query.",
        ));
    }
    let operation = match method {
        Method::GET => Operation::Retrieve,
        Method::DELETE => Operation::Delete,
        Method::POST => Operation::Cancel,
        _ => return Err(GatewayError::bad_request("Unsupported Responses method.")),
    };
    crate::http::native_session::normalize_headers(&mut headers)?;
    let auth = build_public_auth_request(
        &state.config,
        token.as_deref(),
        &headers,
        original.path(),
        method.as_str(),
    );
    let ctx = prepare_context(
        serde_json::json!({"input":[]}),
        token,
        HashMap::new(),
        &headers,
        &state,
    )?;
    if query.stream == Some(true) {
        return super::respond(
            Box::pin(stored_responses::resume(
                ctx,
                auth,
                id,
                query.starting_after,
                state,
            ))
            .await?,
        );
    }
    Box::pin(stored_responses::run(
        ctx,
        auth,
        Request::Existing { id, operation },
        state,
    ))
    .await
    .map(|body| Json(body).into_response())
}
