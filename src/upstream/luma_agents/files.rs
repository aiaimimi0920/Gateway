//! Bounded single-attempt file management. The caller owns cancellation; no detached writes.
use crate::{
    error::GatewayError, protocol::luma_agents::files::Prepared,
    routing::candidate::ProviderAccountPayload,
};
use serde_json::Value;

pub(crate) async fn execute(
    client: &rquest::Client,
    payload: &ProviderAccountPayload,
    operation: Prepared,
) -> Result<Value, GatewayError> {
    #[cfg(not(feature = "line-lumalabs-agents-official-api"))]
    {
        let _ = (client, payload, operation);
        Err(
            GatewayError::service_unavailable("Luma Agents 官方 API 执行线未编译。")
                .with_code("implementation_line_disabled"),
        )
    }
    #[cfg(feature = "line-lumalabs-agents-official-api")]
    {
        tokio::time::timeout(
            match &operation {
                Prepared::Presigned { metadata, .. } => {
                    crate::protocol::luma_agents::file_upload::upload_timeout(metadata.size_bytes)
                }
                _ => std::time::Duration::from_secs(60),
            },
            Box::pin(send(client, payload, operation)),
        )
        .await
        .map_err(|_| uncertain())?
    }
}

#[cfg(feature = "line-lumalabs-agents-official-api")]
async fn send(
    client: &rquest::Client,
    payload: &ProviderAccountPayload,
    operation: Prepared,
) -> Result<Value, GatewayError> {
    use super::{
        files_output as output,
        transport::{read, Material},
    };
    use crate::error::ErrorKind;
    use crate::protocol::luma_agents::{failure, BASE};
    use serde_json::json;
    let material = Material::new(payload)?;
    let operation = match operation {
        Prepared::Presigned { metadata, bytes } => {
            return Box::pin(super::files_upload::send(
                client, &material, metadata, bytes,
            ))
            .await
        }
        operation => operation,
    };
    let path = match &operation {
        Prepared::Get(id) | Prepared::Delete(id) => format!("{BASE}/files/{id}"),
        _ => format!("{BASE}/files"),
    };
    #[cfg(test)]
    let path = super::test_support::target(&path);
    let (request, expected) = match &operation {
        Prepared::Presigned { .. } => unreachable!("presigned upload dispatched above"),
        Prepared::List(cursor) => {
            let mut query = vec![("limit", "25")];
            if let Some(cursor) = cursor {
                query.push(("cursor", cursor.as_str()));
            }
            (client.get(&path).query(&query), 200)
        }
        Prepared::Get(_) => (client.get(&path), 200),
        Prepared::Delete(_) => (client.delete(&path), 204),
        Prepared::Upload {
            filename,
            purpose,
            bytes,
        } => {
            let boundary = format!("gateway-{}", uuid::Uuid::new_v4());
            let mut body = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"purpose\"\r\n\r\n{}\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\nContent-Type: application/octet-stream\r\n\r\n", purpose.as_str()).into_bytes();
            body.extend_from_slice(bytes);
            body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
            (
                client
                    .post(&path)
                    .header(
                        "content-type",
                        format!("multipart/form-data; boundary={boundary}"),
                    )
                    .body(body),
                201,
            )
        }
    };
    let response = request
        .default_headers(false)
        .headers(material.headers())
        .redirect(rquest::redirect::Policy::none())
        .send()
        .await
        .map_err(|_| uncertain())?;
    let status = response.status().as_u16();
    if status != expected {
        return Err(failure(
            match status {
                401 | 403 => ErrorKind::Authentication,
                429 => ErrorKind::RateLimit,
                400 | 404 | 413 | 415 | 422 => ErrorKind::BadRequest,
                _ => ErrorKind::ServerError,
            },
            super::transport::public_failure_status(status),
            "luma_files_http",
            "Luma Files 请求失败；正文已省略。上传或删除请先刷新列表确认结果，不自动重试。",
        ));
    }
    if let Prepared::Delete(id) = &operation {
        return Ok(json!({"action":"delete","id":id}));
    }
    let mime = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim();
    if !mime.eq_ignore_ascii_case("application/json") {
        return Err(output::invalid());
    }
    let bytes = read(response, 256 * 1024).await.map_err(|_| uncertain())?;
    let value = serde_json::from_slice(&bytes).map_err(|_| output::invalid())?;
    match operation {
        Prepared::Presigned { .. } => unreachable!("presigned upload dispatched above"),
        Prepared::List(_) => output::listed(value),
        Prepared::Get(id) => output::retrieved(value, &id),
        Prepared::Upload { .. } => output::uploaded(value),
        Prepared::Delete(_) => unreachable!("DELETE returned above"),
    }
}
#[cfg(feature = "line-lumalabs-agents-official-api")]
pub(super) fn uncertain() -> GatewayError {
    crate::protocol::luma_agents::failure(
        crate::error::ErrorKind::Network,
        502,
        "luma_files_incomplete",
        "Luma Files 请求未完成；上传或删除可能已生效，请先刷新列表确认结果。",
    )
}
