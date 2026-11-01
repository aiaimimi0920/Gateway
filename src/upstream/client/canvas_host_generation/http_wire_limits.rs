//! Admission applies while serializing, including nested JSON and final form expansion.
use super::*;
use std::io::{self, Write};

struct LimitedJson {
    bytes: Vec<u8>,
    limit: usize,
}

impl Write for LimitedJson {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.bytes.len().saturating_add(bytes.len()) > self.limit {
            return Err(io::Error::other("Canvas JSON limit reached"));
        }
        self.bytes
            .try_reserve(bytes.len())
            .map_err(io::Error::other)?;
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(super) fn json<T: serde::Serialize>(value: &T, limit: usize) -> Result<String, GatewayError> {
    let mut writer = LimitedJson {
        bytes: Vec::new(),
        limit,
    };
    serde_json::to_writer(&mut writer, value).map_err(|_| too_large())?;
    String::from_utf8(writer.bytes).map_err(|_| too_large())
}

fn too_large() -> GatewayError {
    invalid(
        "Canvas HTTP encoded request exceeds its size limit.",
        "gemini_canvas_http_request_too_large",
    )
}

fn append_form(result: &mut String, value: &str, limit: usize) -> Result<(), GatewayError> {
    for encoded in url::form_urlencoded::byte_serialize(value.as_bytes()) {
        if result.len().saturating_add(encoded.len()) > limit {
            return Err(too_large());
        }
        result.try_reserve(encoded.len()).map_err(|_| too_large())?;
        result.push_str(encoded);
    }
    Ok(())
}

pub(in crate::upstream::client::canvas_host_generation::http) fn form(
    packed: &str,
    token: Option<&str>,
) -> Result<String, GatewayError> {
    const LIMIT: usize = 64 * 1024 * 1024;
    let mut result = "f.req=".to_string();
    append_form(&mut result, packed, LIMIT)?;
    if let Some(token) = token.filter(|token| !token.is_empty()) {
        if token.len() > 8192 {
            return Err(too_large());
        }
        result.push_str("&at=");
        append_form(&mut result, token, LIMIT)?;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escaping_and_url_expansion_are_rejected_during_writes() {
        assert!(json(&"\\\\\\\\\\\\", 8).is_err());
        let mut output = String::new();
        assert!(append_form(&mut output, "//////", 8).is_err());
        assert!(output.len() <= 8);
        let result = form("quoted \"body\"", Some("token&value")).unwrap();
        let decoded: std::collections::HashMap<_, _> =
            url::form_urlencoded::parse(result.as_bytes())
                .into_owned()
                .collect();
        assert_eq!(decoded["f.req"], "quoted \"body\"");
        assert_eq!(decoded["at"], "token&value");
    }
}
