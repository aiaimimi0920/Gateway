use super::*;

#[test]
fn protocol_errors_preserve_display_and_json_io_source() {
    let error = ProtocolStreamError::invalid_data("bounded protocol failure");
    assert_eq!(
        error.to_string(),
        "error decoding response body: bounded protocol failure"
    );
    let source = error
        .source()
        .unwrap()
        .downcast_ref::<serde_json::Error>()
        .unwrap();
    assert_eq!(
        source.io_error_kind(),
        Some(std::io::ErrorKind::InvalidData)
    );
}

#[test]
fn protocol_boundary_preserves_text_without_impersonating_transport() {
    let error: StreamError<rquest::Error> =
        ProtocolStreamError::invalid_data("bounded protocol failure").into();
    assert!(matches!(error, StreamError::Protocol(_)));
    assert_eq!(
        error.to_string(),
        "error decoding response body: bounded protocol failure"
    );
}

#[test]
fn typed_protocol_variant_preserves_serialization_source_without_transport() {
    let source = serde_json::from_str::<serde_json::Value>("{").unwrap_err();
    let message = source.to_string();
    let error: StreamError<std::io::Error> = ProtocolStreamError::from(source).into();
    assert!(matches!(error, StreamError::Protocol(_)));
    assert_eq!(
        error.source().unwrap().source().unwrap().to_string(),
        message
    );
}

#[test]
fn typed_transport_variant_keeps_original_error_and_source_identity() {
    let error = StreamError::Transport(std::io::Error::new(
        std::io::ErrorKind::TimedOut,
        "body deadline",
    ));
    let StreamError::Transport(transport) = &error else {
        panic!("expected transport")
    };
    assert_eq!(transport.kind(), std::io::ErrorKind::TimedOut);
    // A dyn Error pointer also contains a vtable, which may be duplicated across
    // codegen units. Compare the concrete source object, not trait metadata.
    let source = error
        .source()
        .unwrap()
        .downcast_ref::<std::io::Error>()
        .unwrap();
    assert!(std::ptr::eq(source, transport));
    assert_eq!(error.to_string(), "body deadline");
}
