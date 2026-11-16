//! Isolated S3 client with explicit credentials and conditional writes; no ambient credential chain.
use super::{Destination, Download, DEADLINE, LIMIT};
use aws_sdk_s3::{
    config::{retry::RetryConfig, timeout::TimeoutConfig, Credentials, Region},
    primitives::ByteStream,
    Client,
};

fn client(destination: &Destination) -> (Client, &str, &str) {
    let Destination::S3 {
        endpoint,
        region,
        bucket,
        key,
        access_key_id,
        secret_access_key,
        session_token,
    } = destination
    else {
        unreachable!()
    };
    let config = aws_sdk_s3::config::Builder::new()
        .behavior_version_latest()
        .endpoint_url(endpoint)
        .region(Region::new(region.clone()))
        .force_path_style(true)
        .credentials_provider(Credentials::new(
            access_key_id,
            secret_access_key,
            session_token.clone(),
            None,
            "gateway-backup",
        ))
        .retry_config(RetryConfig::standard().with_max_attempts(1))
        .timeout_config(
            TimeoutConfig::builder()
                .operation_timeout(DEADLINE)
                .connect_timeout(std::time::Duration::from_secs(10))
                .build(),
        )
        .build();
    (Client::from_conf(config), bucket, key)
}

pub(super) async fn upload(
    destination: &Destination,
    bytes: Vec<u8>,
    expected: Option<&str>,
) -> Result<(), String> {
    let (client, bucket, key) = client(destination);
    let mut request = client
        .put_object()
        .bucket(bucket)
        .key(key)
        .content_type("application/octet-stream")
        .body(ByteStream::from(bytes));
    request = match expected {
        Some(tag) => request.if_match(tag),
        None => request.if_none_match("*"),
    };
    request.send().await.map_err(|_| {
        "S3 upload failed or revision conflicted; check remote state before retrying".to_owned()
    })?;
    Ok(())
}

pub(super) async fn download(destination: &Destination) -> Result<Download, String> {
    let (client, bucket, key) = client(destination);
    let response = client
        .get_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await
        .map_err(|_| "S3 download failed")?;
    if response
        .content_length()
        .is_some_and(|n| n < 0 || n > LIMIT as i64)
    {
        return Err("Remote backup exceeds size limit".into());
    }
    let revision = response
        .e_tag()
        .filter(|value| !value.is_empty())
        .ok_or("Missing S3 backup revision")?
        .to_owned();
    let mut body = response.body;
    let mut bytes = Vec::new();
    while let Some(chunk) = body.next().await {
        let chunk = chunk.map_err(|_| "Cannot read S3 backup")?;
        if chunk.len() > LIMIT.saturating_sub(bytes.len()) {
            return Err("Remote backup exceeds size limit".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(Download { bytes, revision })
}
