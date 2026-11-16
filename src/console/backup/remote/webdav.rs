//! A single configured WebDAV object; never create collections or blindly overwrite a backup.
use super::{client, read, revision, Destination, Download, LIMIT};

pub(super) async fn upload(
    destination: &Destination,
    bytes: Vec<u8>,
    expected: Option<&str>,
) -> Result<(), String> {
    let Destination::Webdav {
        url,
        username,
        password,
    } = destination
    else {
        unreachable!()
    };
    let mut request = client()?
        .put(url)
        .basic_auth(username, Some(password))
        .header("content-type", "application/octet-stream")
        .body(bytes);
    request = match expected {
        Some(tag) => request.header("if-match", tag),
        None => request.header("if-none-match", "*"),
    };
    let response = request
        .send()
        .await
        .map_err(|_| "WebDAV upload failed; check remote state before retrying")?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(format!("WebDAV upload returned HTTP {}; existing backups were not intentionally replaced without a matching revision", response.status().as_u16()))
    }
}

pub(super) async fn download(destination: &Destination) -> Result<Download, String> {
    let Destination::Webdav {
        url,
        username,
        password,
    } = destination
    else {
        unreachable!()
    };
    let response = client()?
        .get(url)
        .basic_auth(username, Some(password))
        .send()
        .await
        .map_err(|_| "WebDAV download failed")?;
    let tag = revision(&response)?;
    Ok(Download {
        bytes: read(response, LIMIT).await?,
        revision: tag,
    })
}
