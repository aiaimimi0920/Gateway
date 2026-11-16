//! Pointer-sized construction is a regression guard, not a substitute for the real default-stack public tests.
use super::*;
pub(super) fn future_size<F>(_: impl FnOnce() -> F) -> usize {
    std::mem::size_of::<F>()
}
#[test]
fn vertex_express_dispatch_future_ownership_is_pointer_sized() {
    let payload: ProviderAccountPayload = serde_json::from_value(json!({"adapter":"gemini_api_compatible",
        "base_url":crate::protocol::vertex_express::BASE_URL,"api_key":"synthetic-key",
        "auth_mode":crate::protocol::vertex_express::AUTH_MODE,"auth_header_name":"x-goog-api-key"})).unwrap();
    let request =
        openai::normalize_chat_completions(json!({"messages":[{"role":"user","content":"hello"}]}))
            .unwrap();
    let client = UpstreamClient::new(5);
    eprintln!("unboxed buffered={}, unboxed streaming={}, direct stream={}, wire stream={}, native claude={}",
        super::request_plan_part_2::vertex_express_unboxed_buffered_size(&client, &payload, &request),
        super::streaming::vertex_express_unboxed_streaming_size(&client, &payload, &request),
        future_size(|| client.execute_stream(&payload, &request, "gemini-contract", None)),
        future_size(|| client.execute_stream_with_wire(&payload, &request, "gemini-contract", None, None)),
        future_size(|| client.send_native_claude(&payload, &request, "gemini-contract", None, None)));
    let buffered = client.execute_with_provider_account_id(
        "provider",
        &payload,
        &request,
        "gemini-contract",
        None,
    );
    let streaming = client.execute_stream_with_provider_account_id(
        "provider",
        &payload,
        &request,
        "gemini-contract",
        None,
    );
    assert!(std::mem::size_of_val(&buffered) <= 2 * std::mem::size_of::<usize>());
    assert!(std::mem::size_of_val(&streaming) <= 2 * std::mem::size_of::<usize>());
}
