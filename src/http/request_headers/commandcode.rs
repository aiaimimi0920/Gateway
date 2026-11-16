//! Public privacy controls are strict: malformed or repeated values cannot mean opt-out.
use super::*;

pub(super) fn capture(ctx: &mut PipelineContext, headers: &HeaderMap) -> Result<(), GatewayError> {
    let name = crate::protocol::commandcode::zdr::HEADER;
    let mut values = headers.get_all(name).iter();
    if let Some(value) = values.next() {
        if value.as_bytes() != b"1" || values.next().is_some() {
            return Err(
                GatewayError::bad_request("x-cmd-zdr must occur once with value 1.")
                    .with_code("invalid_commandcode_zdr"),
            );
        }
        ctx.request_headers.insert(name.into(), "1".into());
    }
    Ok(())
}
