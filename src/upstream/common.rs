use std::time::Duration;

use rquest::header::{HeaderMap, HeaderName, HeaderValue};
use rquest::Method;
use rquest::{Client, RequestBuilder};
use serde_json::Value;

use crate::protocol::canonical::EndpointKind;
use crate::upstream::openai_compatible_common::decode_raw_request_body;

#[derive(Debug)]
pub struct RequestPlan {
    pub method: Method,
    pub url: String,
    pub query: Vec<(String, String)>,
    pub body: Option<Value>,
    pub response_kind: EndpointKind,
}

pub fn insert_header_map_value(headers: &mut HeaderMap, name: &str, value: &str) {
    if let (Ok(header_name), Ok(header_value)) = (
        HeaderName::from_bytes(name.as_bytes()),
        HeaderValue::from_str(value),
    ) {
        headers.insert(header_name, header_value);
    }
}

pub fn merge_model_and_stream_into_body(body: Value, model: &str, stream: Option<bool>) -> Value {
    let mut raw = body;
    if let Value::Object(ref mut map) = raw {
        map.insert("model".to_string(), Value::String(model.to_string()));
        if let Some(stream) = stream {
            map.insert("stream".to_string(), Value::Bool(stream));
        }
    }
    raw
}

pub(crate) fn build_request_builder_from_plan(
    http: &Client,
    timeout: Duration,
    plan: &RequestPlan,
    headers: HeaderMap,
) -> RequestBuilder {
    let mut builder = http
        .request(plan.method.clone(), &plan.url)
        .headers(headers)
        .timeout(timeout);

    if !plan.query.is_empty() {
        builder = builder.query(&plan.query);
    }

    if let Some(body) = &plan.body {
        if let Some((content_type, bytes)) = decode_raw_request_body(body) {
            builder = builder
                .header(rquest::header::CONTENT_TYPE, content_type)
                .body(bytes);
        } else {
            builder = builder.json(body);
        }
    }

    builder
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    fn make_client() -> Client {
        Client::new()
    }

    #[test]
    fn build_request_builder_from_plan_preserves_query_headers_and_json_body() {
        let client = make_client();
        let plan = RequestPlan {
            method: Method::POST,
            url: "https://example.com/v1/test".to_string(),
            query: vec![("alpha".to_string(), "1".to_string())],
            body: Some(serde_json::json!({"hello":"world"})),
            response_kind: EndpointKind::ChatCompletions,
        };
        let mut headers = HeaderMap::new();
        headers.insert(
            rquest::header::HeaderName::from_static("x-demo"),
            HeaderValue::from_static("demo"),
        );

        let request =
            build_request_builder_from_plan(&client, Duration::from_secs(5), &plan, headers)
                .build()
                .expect("request should build");

        assert_eq!(request.method(), Method::POST);
        assert_eq!(
            request.url().as_str(),
            "https://example.com/v1/test?alpha=1"
        );
        assert_eq!(
            request
                .headers()
                .get("x-demo")
                .and_then(|value| value.to_str().ok()),
            Some("demo")
        );
        assert_eq!(
            request
                .headers()
                .get(rquest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("application/json")
        );
        assert_eq!(
            request.body().and_then(|body| body.as_bytes()),
            Some(br#"{"hello":"world"}"#.as_slice())
        );
    }

    #[test]
    fn build_request_builder_from_plan_prefers_raw_request_body_contract() {
        let client = make_client();
        let plan = RequestPlan {
            method: Method::POST,
            url: "https://example.com/v1/raw".to_string(),
            query: Vec::new(),
            body: Some(serde_json::json!({
                "__gateway_raw_body_content_type": "application/x.custom",
                "__gateway_raw_body_base64": base64::engine::general_purpose::STANDARD.encode("raw-body")
            })),
            response_kind: EndpointKind::ChatCompletions,
        };

        let request = build_request_builder_from_plan(
            &client,
            Duration::from_secs(5),
            &plan,
            HeaderMap::new(),
        )
        .build()
        .expect("request should build");

        assert_eq!(request.method(), Method::POST);
        assert_eq!(request.url().as_str(), "https://example.com/v1/raw");
        assert_eq!(
            request
                .headers()
                .get(rquest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("application/x.custom")
        );
        assert_eq!(
            request.body().and_then(|body| body.as_bytes()),
            Some(b"raw-body".as_slice())
        );
    }
}
