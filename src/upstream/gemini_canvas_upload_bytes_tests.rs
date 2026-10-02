use super::select_upload_bytes;
use http_body_util::BodyExt;

#[test]
fn encoded_upload_storage_is_reused_by_the_request_body() {
    for capacity in [4, 64] {
        let mut encoded = Vec::with_capacity(capacity);
        encoded.extend_from_slice(&[0, 128, 255, 3]);
        let original_ptr = encoded.as_ptr();
        let selected = select_upload_bytes(Some(encoded), &[9, 9, 9]);

        assert_eq!(selected.as_ref(), &[0, 128, 255, 3]);
        assert_eq!(selected.as_ptr(), original_ptr);
        let request_body = rquest::Body::from(selected.clone());
        // 新客户端公开 HttpBody，而非借用 accessor；释放外部 owner 后验证真实发送字节和原分配。
        drop(selected);
        let request_bytes = futures::executor::block_on(request_body.collect())
            .unwrap()
            .to_bytes();
        assert_eq!(request_bytes.as_ptr(), original_ptr);
        assert_eq!(request_bytes.as_ref(), &[0, 128, 255, 3]);
    }
}

#[test]
fn fallback_is_owned_without_a_second_request_payload_copy() {
    let mut fallback = vec![1, 128, 255, 0];
    let selected = select_upload_bytes(None, &fallback);
    assert_eq!(fallback, [1, 128, 255, 0]);
    assert_eq!(selected.as_ref(), fallback.as_slice());
    assert_ne!(selected.as_ptr(), fallback.as_ptr());

    let selected_ptr = selected.as_ptr();
    let request_body = rquest::Body::from(selected.clone());
    fallback.fill(9);
    drop(selected);
    let request_bytes = futures::executor::block_on(request_body.collect())
        .unwrap()
        .to_bytes();
    assert_eq!(request_bytes.as_ptr(), selected_ptr);
    assert_eq!(request_bytes.as_ref(), &[1, 128, 255, 0]);
}

#[test]
fn empty_encoded_output_does_not_select_nonempty_fallback() {
    let encoded = select_upload_bytes(Some(Vec::new()), &[1, 2, 3]);
    assert!(encoded.is_empty());
    let fallback = select_upload_bytes(None, &[]);
    assert!(fallback.is_empty());
    for bytes in [encoded, fallback] {
        let request_body = rquest::Body::from(bytes);
        let request_bytes = futures::executor::block_on(request_body.collect())
            .unwrap()
            .to_bytes();
        assert!(request_bytes.is_empty());
    }
}
