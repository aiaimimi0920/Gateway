//! Test-only loopback replacement happens after the normal product plan and final headers are validated.
use crate::{routing::candidate::ProviderAccountPayload, upstream::common::RequestPlan};
mod server;
pub(crate) use server::Server;
tokio::task_local! {static ORIGIN:String;}
pub(crate) async fn within<T>(origin: &str, future: impl std::future::Future<Output = T>) -> T {
    let url = url::Url::parse(origin).unwrap();
    assert_eq!(url.scheme(), "http");
    assert_eq!(url.host_str(), Some("127.0.0.1"));
    assert!(url.port().is_some());
    crate::request_wait::transport::within(false, ORIGIN.scope(origin.to_owned(), future)).await
}
pub(super) fn transport_plan(
    payload: &ProviderAccountPayload,
    plan: &RequestPlan,
) -> Option<RequestPlan> {
    if !crate::protocol::kiro_api_key::owns_payload(payload)
        && !crate::protocol::kiro_oauth::runtime::owns_payload(payload)
    {
        return None;
    }
    Some(
        ORIGIN
            .try_with(|origin| RequestPlan {
                method: plan.method.clone(),
                url: format!("{origin}/"),
                query: plan.query.clone(),
                body: plan.body.clone(),
                response_kind: plan.response_kind,
            })
            .expect("Native runtime tests must explicitly select loopback transport"),
    )
}

pub(crate) fn event_frame(text: &str) -> Vec<u8> {
    event(
        "assistantResponseEvent",
        serde_json::json!({"content":text}),
    )
}
pub(crate) fn event(event_type: &str, body: serde_json::Value) -> Vec<u8> {
    frame(
        &[(":message-type", "event"), (":event-type", event_type)],
        &serde_json::to_vec(&body).unwrap(),
    )
}
pub(crate) fn error_frame(message: &str) -> Vec<u8> {
    frame(
        &[
            (":message-type", "error"),
            (":error-code", "ProviderFailure"),
        ],
        message.as_bytes(),
    )
}
pub(crate) fn frame(fields: &[(&str, &str)], body: &[u8]) -> Vec<u8> {
    let mut headers = Vec::new();
    for (name, value) in fields {
        headers.push(name.len() as u8);
        headers.extend_from_slice(name.as_bytes());
        headers.push(7);
        headers.extend_from_slice(&(value.len() as u16).to_be_bytes());
        headers.extend_from_slice(value.as_bytes());
    }
    let mut frame = Vec::new();
    frame.extend_from_slice(&((16 + headers.len() + body.len()) as u32).to_be_bytes());
    frame.extend_from_slice(&(headers.len() as u32).to_be_bytes());
    let crc = crc::Crc::<u32>::new(&crc::CRC_32_ISO_HDLC);
    frame.extend_from_slice(&crc.checksum(&frame).to_be_bytes());
    frame.extend(headers);
    frame.extend_from_slice(body);
    frame.extend_from_slice(&crc.checksum(&frame).to_be_bytes());
    frame
}
