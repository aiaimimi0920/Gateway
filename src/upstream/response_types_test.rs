use crate::upstream::response_types::{BinaryUpstreamResponse, UpstreamStreamingResponse};
use futures::StreamExt;

#[test]
fn binary_upstream_response_preserves_body_content_type_and_extra_headers() {
    let response = BinaryUpstreamResponse {
        body: bytes::Bytes::from_static(b"audio-bytes"),
        content_type: Some("audio/mpeg".to_string()),
        extra_headers: vec![("content-disposition".to_string(), "attachment".to_string())],
    };

    assert_eq!(response.body.as_ref(), b"audio-bytes");
    assert_eq!(response.content_type.as_deref(), Some("audio/mpeg"));
    assert_eq!(
        response.extra_headers,
        vec![("content-disposition".to_string(), "attachment".to_string())]
    );
}

#[tokio::test]
async fn upstream_streaming_response_bytes_variant_preserves_streamed_bytes() {
    let chunks = vec![
        Ok(bytes::Bytes::from_static(b"data: first\n\n")),
        Ok(bytes::Bytes::from_static(b"data: [DONE]\n\n")),
    ];
    let response =
        UpstreamStreamingResponse::Bytes(Box::pin(futures::stream::iter(chunks.into_iter())));

    let UpstreamStreamingResponse::Bytes(mut stream) = response else {
        panic!("expected bytes streaming variant");
    };

    let first = stream
        .next()
        .await
        .expect("first chunk exists")
        .expect("first chunk is ok");
    let second = stream
        .next()
        .await
        .expect("second chunk exists")
        .expect("second chunk is ok");

    assert_eq!(first.as_ref(), b"data: first\n\n");
    assert_eq!(second.as_ref(), b"data: [DONE]\n\n");
    assert!(stream.next().await.is_none());
}
