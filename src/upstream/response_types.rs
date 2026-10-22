#[derive(Debug)]
pub struct BinaryUpstreamResponse {
    pub body: bytes::Bytes,
    pub content_type: Option<String>,
    pub extra_headers: Vec<(String, String)>,
}

pub enum UpstreamStreamingResponse {
    Http(rquest::Response),
    Native(reqwest::Response),
    NativeEvents(
        std::pin::Pin<
            Box<
                dyn futures::Stream<
                        Item = Result<
                            bytes::Bytes,
                            crate::protocol::stream_error::StreamError<rquest::Error>,
                        >,
                    > + Send,
            >,
        >,
    ),
    Bytes(
        std::pin::Pin<Box<dyn futures::Stream<Item = Result<bytes::Bytes, rquest::Error>> + Send>>,
    ),
}
