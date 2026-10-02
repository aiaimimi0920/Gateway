use neuro_gateway::upstream::client::UpstreamClient;
use rquest::header::HeaderValue;

const KEY_ERROR: &str = "JINA_API_KEY must be set to a non-empty ASCII token of at most 4096 bytes";

// Reject invalid input before constructing a client; errors never contain the key.
fn authorization_from_key(
    key: Result<String, std::env::VarError>,
) -> Result<HeaderValue, &'static str> {
    let key = key.map_err(|_| KEY_ERROR)?;
    if key.is_empty() || key.len() > 4096 || !key.bytes().all(|byte| byte.is_ascii_graphic()) {
        return Err(KEY_ERROR);
    }
    let mut value = HeaderValue::from_str(&format!("Bearer {key}")).map_err(|_| KEY_ERROR)?;
    value.set_sensitive(true);
    Ok(value)
}

#[tokio::main]
async fn main() -> Result<(), &'static str> {
    let authorization = authorization_from_key(std::env::var("JINA_API_KEY"))?;
    let client = UpstreamClient::new(30);
    let http = client.client().clone();
    for url in ["https://s.jina.ai/search", "https://r.jina.ai/"] {
        let body = if url.contains("s.jina.ai") {
            serde_json::json!({"q": "OpenAI"})
        } else {
            serde_json::json!({"url": "http://example.com"})
        };
        match http
            .post(url)
            .header("Authorization", authorization.clone())
            .header("Accept", "application/json")
            .json(&body)
            .send()
            .await
        {
            Ok(resp) => {
                println!(
                    "{} => {} {:?}",
                    url,
                    resp.status(),
                    resp.headers().get("content-type")
                );
            }
            Err(err) => {
                println!("{} => ERR {}", url, err);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_key_is_rejected_without_secret_details() {
        assert_eq!(
            authorization_from_key(Err(std::env::VarError::NotPresent)),
            Err(KEY_ERROR)
        );
    }

    #[test]
    fn non_unicode_key_is_rejected_without_secret_details() {
        let error = std::env::VarError::NotUnicode("synthetic-private-value".into());
        assert_eq!(authorization_from_key(Err(error)), Err(KEY_ERROR));
    }

    #[test]
    fn empty_and_whitespace_keys_are_rejected() {
        for key in ["", " ", "\t", "synthetic key", " synthetic", "synthetic "] {
            assert_eq!(authorization_from_key(Ok(key.into())), Err(KEY_ERROR));
        }
    }

    #[test]
    fn header_injection_and_non_ascii_are_rejected() {
        for key in ["synthetic\r\nX-Injected: yes", "synthetic\0", "synthetic-é"] {
            assert_eq!(authorization_from_key(Ok(key.into())), Err(KEY_ERROR));
        }
    }

    #[test]
    fn oversized_key_is_rejected() {
        assert_eq!(authorization_from_key(Ok("x".repeat(4097))), Err(KEY_ERROR));
    }

    #[test]
    fn valid_synthetic_key_is_shared_and_sensitive() {
        let value = authorization_from_key(Ok("synthetic-jina-key".into())).unwrap();
        assert_eq!(value.as_bytes(), b"Bearer synthetic-jina-key");
        assert!(value.is_sensitive());
        assert!(value.clone().is_sensitive());
        assert!(!format!("{value:?}").contains("synthetic-jina-key"));
    }
}
