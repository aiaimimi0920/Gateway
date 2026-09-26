use super::select_upload_bytes;

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
        let request_bytes = request_body.as_bytes().expect("reusable upload body");
        assert_eq!(request_bytes.as_ptr(), original_ptr);
        assert_eq!(request_bytes, selected.as_ref());

        drop(selected);
        assert_eq!(request_body.as_bytes(), Some([0, 128, 255, 3].as_slice()));
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
    assert_eq!(
        request_body
            .as_bytes()
            .expect("reusable upload body")
            .as_ptr(),
        selected_ptr
    );
    fallback.fill(9);
    drop(selected);
    assert_eq!(request_body.as_bytes(), Some([1, 128, 255, 0].as_slice()));
}

#[test]
fn empty_encoded_output_does_not_select_nonempty_fallback() {
    let encoded = select_upload_bytes(Some(Vec::new()), &[1, 2, 3]);
    assert!(encoded.is_empty());
    let fallback = select_upload_bytes(None, &[]);
    assert!(fallback.is_empty());
    for bytes in [encoded, fallback] {
        let request_body = rquest::Body::from(bytes);
        assert_eq!(request_body.as_bytes(), Some([].as_slice()));
    }
}
