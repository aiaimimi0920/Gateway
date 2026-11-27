//! Cursor-only direct native response path; ordinary provider translation is unchanged.
use crate::{
    error::GatewayError,
    pipeline::PipelineOutput,
    protocol::{
        canonical::{EndpointKind, ProtocolFamily},
        cursor_remote::{native_publication, runtime::Publication},
    },
};
use axum::{
    response::{IntoResponse, Response},
    Json,
};

pub(super) fn response(
    output: PipelineOutput,
    mut publication: tokio::sync::oneshot::Receiver<Publication>,
    family: ProtocolFamily,
) -> Result<Response, GatewayError> {
    let response = match output {
        PipelineOutput::Json(value) => Json(value).into_response(),
        PipelineOutput::Sse(stream) => {
            crate::http::sse::into_sse_response(stream, EndpointKind::ChatCompletions)
                .into_response()
        }
        PipelineOutput::Binary(_) => {
            return Err(GatewayError::server_error(
                "unexpected Cursor native binary response",
            ))
        }
    };
    Ok(match publication.try_recv() {
        Ok(guard) => super::completions::cursor_publication::attach_validated(
            response,
            guard,
            move |bytes, sse| native_publication::validate(family, bytes, sse),
        ),
        Err(_) => response,
    })
}
