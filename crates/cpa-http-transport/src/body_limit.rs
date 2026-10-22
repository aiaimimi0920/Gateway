//! Typed, secret-free error preserved through IO and rquest body adapters.
#[derive(Debug)]
pub struct BodyLimitExceeded;

impl std::fmt::Display for BodyLimitExceeded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CPA upstream body exceeded the configured limit")
    }
}
impl std::error::Error for BodyLimitExceeded {}

pub fn is_body_limit_error(error: &(dyn std::error::Error + 'static)) -> bool {
    let mut current = Some(error);
    // Error chains are trusted library objects, but traversal still has a defensive bound.
    for _ in 0..32 {
        let Some(error) = current else {
            return false;
        };
        if error.is::<BodyLimitExceeded>() {
            return true;
        }
        if let Some(inner) = error
            .downcast_ref::<std::io::Error>()
            .and_then(|e| e.get_ref())
        {
            if inner.is::<BodyLimitExceeded>() {
                return true;
            }
        }
        current = error.source();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn type_survives_real_response_body_adapter() {
        let stream = futures::stream::once(async {
            Err::<bytes::Bytes, _>(std::io::Error::other(BodyLimitExceeded))
        });
        let response: rquest::Response =
            http::Response::new(rquest::Body::wrap_stream(stream)).into();
        let error = response.bytes().await.unwrap_err();
        assert!(is_body_limit_error(&error));
        assert!(!is_body_limit_error(&std::io::Error::other(
            "different error"
        )));
    }
}
