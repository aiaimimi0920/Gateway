use super::*;

#[test]
fn build_gemini_canvas_image_edit_upload_response_meta_preserves_start_shape() {
    let meta = build_gemini_canvas_image_edit_upload_response_meta(
        "https://push.clients6.google.com/upload/final",
        Some("text/plain"),
        Some(true),
        "abcdefghijklmnopqrstuvwxyz",
    );

    assert_eq!(
        meta,
        "final_url=https://push.clients6.google.com/upload/final, content_type=text/plain, upload_url_present=true, body_preview=abcdefghijklmnopqrstuvwxyz"
    );
}

#[test]
fn build_gemini_canvas_image_edit_upload_response_meta_preserves_finalize_shape() {
    let meta = build_gemini_canvas_image_edit_upload_response_meta(
        "https://push.clients6.google.com/upload/final",
        None,
        None,
        "",
    );

    assert_eq!(
        meta,
        "final_url=https://push.clients6.google.com/upload/final, content_type=<none>, body_preview=<empty>"
    );
}

#[test]
fn build_gemini_canvas_image_edit_upload_start_request_contract_preserves_shape() {
    let contract = build_gemini_canvas_image_edit_upload_start_request_contract(
        "https://push.clients6.google.com/upload/",
        "source.png",
        "image/png",
        1234,
        "push-id-abcdefghijklmnopqrstuvwxyz",
        "client-pctx-abcdefghijklmnopqrstuvwxyz",
        "https://gemini.google.com/app",
    );

    assert_eq!(
        contract,
        "upload_base_url=https://push.clients6.google.com/upload/, file_name=source.png, mime_type=image/png, bytes=1234, push_id=push-id-abcdefghijklmnop, client_pctx=client-pctx-abcdefghijkl, referer=https://gemini.google.com/app"
    );
}

#[test]
fn build_gemini_canvas_image_edit_upload_start_request_contract_preserves_short_tokens() {
    let contract = build_gemini_canvas_image_edit_upload_start_request_contract(
        "https://push.clients6.google.com/upload/",
        "tiny.png",
        "image/png",
        9,
        "push-1",
        "ctx-1",
        "https://gemini.google.com/app",
    );

    assert_eq!(
        contract,
        "upload_base_url=https://push.clients6.google.com/upload/, file_name=tiny.png, mime_type=image/png, bytes=9, push_id=push-1, client_pctx=ctx-1, referer=https://gemini.google.com/app"
    );
}

#[test]
fn build_gemini_canvas_image_edit_upload_finalize_request_contract_preserves_shape() {
    let contract = build_gemini_canvas_image_edit_upload_finalize_request_contract(
        "https://push.clients6.google.com/upload/session",
        "source.png",
        "image/png",
        1234,
    );

    assert_eq!(
        contract,
        "upload_url=https://push.clients6.google.com/upload/session, file_name=source.png, mime_type=image/png, bytes=1234"
    );
}

#[test]
fn build_gemini_canvas_image_edit_upload_finalize_request_contract_preserves_zero_bytes() {
    let contract = build_gemini_canvas_image_edit_upload_finalize_request_contract(
        "https://push.clients6.google.com/upload/session",
        "empty.bin",
        "application/octet-stream",
        0,
    );

    assert_eq!(
        contract,
        "upload_url=https://push.clients6.google.com/upload/session, file_name=empty.bin, mime_type=application/octet-stream, bytes=0"
    );
}

#[test]
fn append_gemini_canvas_image_edit_upload_contracts_preserves_message_contract() {
    let error = GatewayError::service_unavailable("base message")
        .with_provider("gemini_canvas_compatible")
        .with_code("base_code");

    let updated =
        append_gemini_canvas_image_edit_upload_contracts(error, "request=abc", "response=def");

    assert_eq!(
        updated.message,
        "base message; upload_request_contract=request=abc; upload_response_meta=response=def"
    );
    assert_eq!(updated.code.as_deref(), Some("base_code"));
}

#[test]
fn gemini_canvas_upload_contract_cache_preserves_error_and_snapshot_values() {
    use crate::upstream::gemini_canvas_image_edit_local_helpers::build_gemini_canvas_image_edit_upload_debug_snapshot;
    use std::cell::{Cell, LazyCell};

    for snapshot_first in [false, true] {
        let requests = Cell::new(0);
        let responses = Cell::new(0);
        let request = LazyCell::new(|| {
            requests.set(requests.get() + 1);
            build_gemini_canvas_image_edit_upload_finalize_request_contract(
                "fixture-url",
                "source.png",
                "image/png",
                3,
            )
        });
        let response = LazyCell::new(|| {
            responses.set(responses.get() + 1);
            build_gemini_canvas_image_edit_upload_response_meta(
                "fixture-url",
                Some("text/plain"),
                None,
                "fixture-body",
            )
        });
        let snapshot = || {
            build_gemini_canvas_image_edit_upload_debug_snapshot(
                &request,
                &response,
                "fixture-body",
                None,
                "source.png",
                "image/png",
                3,
                "fixture-hash",
                None,
                None,
                None,
                None,
            )
        };
        assert_eq!((requests.get(), responses.get()), (0, 0));
        if snapshot_first {
            assert_eq!(snapshot()["requestContract"], request.as_str());
        }
        let error = append_gemini_canvas_image_edit_upload_contracts(
            GatewayError::service_unavailable("fixture-error"),
            &request,
            &response,
        );
        assert_eq!(
            error.message,
            "fixture-error; upload_request_contract=upload_url=fixture-url, file_name=source.png, mime_type=image/png, bytes=3; upload_response_meta=final_url=fixture-url, content_type=text/plain, body_preview=fixture-body"
        );
        let cached = snapshot();
        assert_eq!(cached["requestContract"], request.as_str());
        assert_eq!(cached["responseMeta"], response.as_str());
        assert_eq!(cached["responseBody"], "fixture-body");
        assert_eq!((requests.get(), responses.get()), (1, 1));
    }
}
