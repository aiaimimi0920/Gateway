//! Presign -> bounded storage PUT -> complete, in one cancellable future without replay.
use super::{
    files_output::{self as output, invalid},
    transport::{read, Material},
};
use crate::{
    error::GatewayError,
    protocol::luma_agents::{
        file_upload::{upload_timeout, Upload, UploadBody},
        BASE,
    },
    upstream::public_assets,
};
use rquest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::Deserialize;
use serde_json::{json, Value};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use tokio::time::Instant;

#[derive(Deserialize)]
struct Envelope {
    url: String,
    method: String,
    expires_at: String,
    #[serde(default)]
    headers: std::collections::BTreeMap<String, String>,
}

pub(super) async fn send(
    client: &rquest::Client,
    material: &Material,
    metadata: Upload,
    bytes: UploadBody,
) -> Result<Value, GatewayError> {
    let deadline = Instant::now() + upload_timeout(metadata.size_bytes);
    let created = api(client, material, "/files", Some(&metadata)).await?;
    let file = output::file(created.get("file").cloned().ok_or_else(invalid)?)?;
    if created["id"] != file.id
        || created["state"] != file.state
        || file.state != "pending"
        || !file.matches_upload(&metadata)
    {
        return Err(invalid());
    }
    let envelope: Envelope =
        serde_json::from_value(created.get("upload").cloned().ok_or_else(invalid)?)
            .map_err(|_| invalid())?;
    let headers = envelope.headers(&metadata)?;
    // Never finalize until BOTH the exact multipart input and storage PUT have completed.
    // try_join drops the peer future on error; neither half can outlive this request.
    tokio::try_join!(
        async {
            public_assets::upload::put(&envelope.url, headers, bytes.body, bytes.size, deadline)
                .await
                .map_err(|_| super::files::uncertain())
        },
        bytes.completion
    )?;
    // A transport close or failed PUT never triggers complete; only the original file ID can advance.
    let completed = api(
        client,
        material,
        &format!("/files/{}/complete", file.id),
        None,
    )
    .await?;
    let file = output::file(completed)?;
    if created["id"] != file.id || file.state == "deleted" || !file.matches_upload(&metadata) {
        return Err(invalid());
    }
    Ok(json!({"action":"upload","file":file}))
}

impl Envelope {
    fn headers(&self, metadata: &Upload) -> Result<HeaderMap, GatewayError> {
        if self.method != "PUT"
            || self.expires_at.len() > 64
            || OffsetDateTime::parse(&self.expires_at, &Rfc3339)
                .map_or(true, |t| t <= OffsetDateTime::now_utc())
            || self.headers.len() > 8
            || public_assets::checked_url(&self.url).is_err()
        {
            return Err(invalid());
        }
        let mut result = HeaderMap::new();
        for (name, value) in &self.headers {
            let name = HeaderName::from_bytes(name.as_bytes()).map_err(|_| invalid())?;
            // Signed integrity headers are allowed; credentials, cookies, routing and copy-source are not.
            if !matches!(
                name.as_str(),
                "content-type"
                    | "content-md5"
                    | "x-amz-checksum-sha256"
                    | "x-amz-checksum-sha1"
                    | "x-amz-checksum-crc32"
                    | "x-amz-checksum-crc32c"
            ) || value.len() > 1024
                || result.contains_key(&name)
                || (name == "content-type" && value != &metadata.mime_type)
            {
                return Err(invalid());
            }
            let mut value = HeaderValue::from_str(value).map_err(|_| invalid())?;
            value.set_sensitive(true);
            result.insert(name, value);
        }
        Ok(result)
    }
}

async fn api(
    client: &rquest::Client,
    material: &Material,
    path: &str,
    metadata: Option<&Upload>,
) -> Result<Value, GatewayError> {
    let path = format!("{BASE}{path}");
    #[cfg(test)]
    let path = super::test_support::target(&path);
    let mut request = client
        .post(path)
        .default_headers(false)
        .headers(material.headers())
        .redirect(rquest::redirect::Policy::none());
    if let Some(metadata) = metadata {
        request = request.json(metadata);
    }
    let response = request
        .send()
        .await
        .map_err(|_| super::files::uncertain())?;
    if response.status().as_u16() != if metadata.is_some() { 201 } else { 200 } {
        return Err(super::files::uncertain());
    }
    if response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(';').next())
        .is_none_or(|s| !s.trim().eq_ignore_ascii_case("application/json"))
    {
        return Err(invalid());
    }
    let bytes = read(response, 256 * 1024)
        .await
        .map_err(|_| super::files::uncertain())?;
    serde_json::from_slice(&bytes).map_err(|_| invalid())
}
