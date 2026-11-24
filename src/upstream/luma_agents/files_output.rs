//! Whitelisted file metadata; storage URLs, raw errors and arbitrary fields never reach the console.
use crate::{
    error::{ErrorKind, GatewayError},
    protocol::luma_agents::{
        failure,
        files::{file_id, valid_cursor, PAGE_SIZE},
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;

#[derive(Deserialize, Serialize)]
pub(super) struct File {
    pub id: String,
    pub state: String,
    filename: Option<String>,
    mime_type: String,
    size_bytes: u64,
    purpose: String,
    created_at: String,
    expires_at: Option<String>,
    user_id: Option<String>,
}
impl File {
    pub(super) fn matches_upload(
        &self,
        upload: &crate::protocol::luma_agents::file_upload::Upload,
    ) -> bool {
        self.filename.as_deref() == Some(&upload.filename)
            && self.mime_type == upload.mime_type
            && self.size_bytes == upload.size_bytes as u64
            && self.purpose == upload.purpose.as_str()
            && self.user_id == upload.user_id
            && match (&self.expires_at, &upload.expires_at) {
                (None, None) => true,
                (Some(a), Some(b)) => {
                    let format = time::format_description::well_known::Rfc3339;
                    time::OffsetDateTime::parse(a, &format).ok()
                        == time::OffsetDateTime::parse(b, &format).ok()
                }
                _ => false,
            }
    }
}
pub(super) fn file(value: Value) -> Result<File, GatewayError> {
    let file: File = serde_json::from_value(value).map_err(|_| invalid())?;
    if file_id(&file.id).ok().as_deref() != Some(file.id.as_str())
        || !matches!(
            file.state.as_str(),
            "pending" | "ready" | "failed" | "deleted"
        )
        || !matches!(file.purpose.as_str(), "input" | "reference")
        || file.size_bytes > 5 * 1024 * 1024 * 1024
        || file
            .filename
            .as_ref()
            .is_some_and(|v| v.len() > 1024 || v.chars().any(char::is_control))
        || file.mime_type.is_empty()
        || file.mime_type.len() > 255
        || !file.mime_type.bytes().all(|b| b.is_ascii_graphic())
        || file.created_at.len() > 64
        || file
            .user_id
            .as_ref()
            .is_some_and(|s| s.len() > 256 || s.chars().any(char::is_control))
        || file.expires_at.as_ref().is_some_and(|s| {
            s.len() > 64
                || time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339)
                    .is_err()
        })
        || time::OffsetDateTime::parse(
            &file.created_at,
            &time::format_description::well_known::Rfc3339,
        )
        .is_err()
    {
        return Err(invalid());
    }
    Ok(file)
}
pub(super) fn uploaded(mut value: Value) -> Result<Value, GatewayError> {
    if value.get("upload").is_some_and(|upload| !upload.is_null()) {
        return Err(invalid());
    }
    let file = file(value.get_mut("file").ok_or_else(invalid)?.take())?;
    if value["id"] != file.id || value["state"] != file.state {
        return Err(invalid());
    }
    Ok(json!({"action":"upload","file":file}))
}
pub(super) fn retrieved(value: Value, id: &str) -> Result<Value, GatewayError> {
    let file = file(value)?;
    if file.id != id {
        return Err(invalid());
    }
    Ok(json!({"action":"get","file":file}))
}
pub(super) fn listed(value: Value) -> Result<Value, GatewayError> {
    #[derive(Deserialize)]
    struct Page {
        data: Vec<Value>,
        has_more: bool,
        next_cursor: Option<String>,
    }
    let page: Page = serde_json::from_value(value).map_err(|_| invalid())?;
    if page.data.len() > PAGE_SIZE
        || page
            .next_cursor
            .as_deref()
            .is_some_and(|v| !valid_cursor(v))
        || (page.has_more && (page.next_cursor.is_none() || page.data.is_empty()))
    {
        return Err(invalid());
    }
    let files = page
        .data
        .into_iter()
        .map(file)
        .collect::<Result<Vec<_>, _>>()?;
    let mut ids = HashSet::new();
    if files.iter().any(|f| !ids.insert(&f.id)) {
        return Err(invalid());
    }
    Ok(
        json!({"action":"list","data":files,"has_more":page.has_more,
        "next_cursor":if page.has_more { page.next_cursor } else { None }}),
    )
}
pub(super) fn invalid() -> GatewayError {
    failure(
        ErrorKind::ServerError,
        502,
        "luma_files_response",
        "Luma Files 响应不符合合同；正文已省略。",
    )
}
