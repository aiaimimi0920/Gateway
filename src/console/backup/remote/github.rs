//! GitHub Contents API; repository/path/ref are encoded, tokens never enter a URL.
use super::{client, read, Destination, Download, LIMIT};
use base64::{engine::general_purpose::STANDARD, Engine};

fn endpoint<'a>(
    destination: &'a Destination,
    base: &str,
) -> Result<(url::Url, &'a str, &'a str), String> {
    let Destination::Github {
        owner,
        repository,
        branch,
        path,
        token,
    } = destination
    else {
        unreachable!()
    };
    let mut url = url::Url::parse(base).map_err(|_| "Invalid GitHub API URL")?;
    url.path_segments_mut()
        .map_err(|_| "Invalid GitHub API path")?
        .extend(["repos", owner, repository, "contents"])
        .extend(path.split('/'));
    Ok((url, token, branch))
}

pub(super) async fn upload(
    destination: &Destination,
    bytes: Vec<u8>,
    expected: Option<&str>,
) -> Result<(), String> {
    upload_at(destination, bytes, expected, "https://api.github.com").await
}

async fn upload_at(
    destination: &Destination,
    bytes: Vec<u8>,
    expected: Option<&str>,
    base: &str,
) -> Result<(), String> {
    let (url, token, branch) = endpoint(destination, base)?;
    let mut body = serde_json::json!({"message":"Update encrypted Gateway backup", "content":STANDARD.encode(bytes), "branch":branch});
    if let Some(sha) = expected {
        body["sha"] = sha.into();
    }
    let response = client()?
        .put(url)
        .bearer_auth(token)
        .header("accept", "application/vnd.github+json")
        .header("x-github-api-version", "2022-11-28")
        .json(&body)
        .send()
        .await
        .map_err(|_| "GitHub upload failed; check remote state before retrying")?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(format!(
            "GitHub upload returned HTTP {}",
            response.status().as_u16()
        ))
    }
}

pub(super) async fn download(destination: &Destination) -> Result<Download, String> {
    download_at(destination, "https://api.github.com").await
}

async fn download_at(destination: &Destination, base: &str) -> Result<Download, String> {
    let (mut url, token, branch) = endpoint(destination, base)?;
    url.query_pairs_mut().append_pair("ref", branch);
    let client = client()?;
    let response = client
        .get(url.clone())
        .bearer_auth(token)
        .header("accept", "application/vnd.github.object+json")
        .header("x-github-api-version", "2022-11-28")
        .send()
        .await
        .map_err(|_| "GitHub metadata download failed")?;
    let bytes = read(response, 2 * 1024 * 1024).await?;
    let metadata: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| "Invalid GitHub backup metadata")?;
    if metadata["type"].as_str() != Some("file")
        || metadata["size"]
            .as_u64()
            .is_none_or(|size| size > LIMIT as u64)
    {
        return Err("GitHub backup is not a supported file".into());
    }
    let sha = metadata["sha"]
        .as_str()
        .filter(|s| s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or("Invalid GitHub backup revision")?
        .to_owned();
    // Pin the raw read to the blob SHA, so a concurrent branch update cannot mix revision and bytes.
    let Destination::Github {
        owner, repository, ..
    } = destination
    else {
        unreachable!()
    };
    let mut blob_url = url::Url::parse(base).map_err(|_| "Invalid GitHub API URL")?;
    blob_url
        .path_segments_mut()
        .map_err(|_| "Invalid GitHub API path")?
        .extend(["repos", owner, repository, "git", "blobs", &sha]);
    let response = client
        .get(blob_url)
        .bearer_auth(token)
        .header("accept", "application/vnd.github.raw+json")
        .header("x-github-api-version", "2022-11-28")
        .send()
        .await
        .map_err(|_| "GitHub backup download failed")?;
    Ok(Download {
        bytes: read(response, LIMIT).await?,
        revision: sha,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        extract::Request,
        http::{Response, StatusCode},
        routing::any,
        Router,
    };

    #[tokio::test]
    async fn github_pins_download_and_uses_sha_for_conditional_update() {
        let sha = "a".repeat(40);
        let fixture_sha = sha.clone();
        let app = Router::new().fallback(any(move |request: Request| {
            let sha = fixture_sha.clone();
            async move {
                assert_eq!(request.headers()["authorization"], "Bearer fixture-token");
                if request.method() == "PUT" {
                    let bytes = to_bytes(request.into_body(), 4096).await.unwrap();
                    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                    assert_eq!(body["sha"], sha);
                    assert_eq!(body["branch"], "backup/main");
                    assert_eq!(
                        STANDARD.decode(body["content"].as_str().unwrap()).unwrap(),
                        b"fixture-ciphertext"
                    );
                    return Response::builder()
                        .status(StatusCode::OK)
                        .body(Body::empty())
                        .unwrap();
                }
                if request.uri().path().contains("/git/blobs/") {
                    assert!(request.uri().path().ends_with(&sha));
                    assert_eq!(
                        request.headers()["accept"],
                        "application/vnd.github.raw+json"
                    );
                    return Response::new(Body::from("fixture-ciphertext"));
                }
                assert!(request.uri().query().unwrap().contains("ref=backup%2Fmain"));
                Response::new(Body::from(
                    serde_json::json!({"type":"file", "sha":sha, "size":18}).to_string(),
                ))
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let destination = Destination::Github {
            owner: "fixture".into(),
            repository: "backup".into(),
            branch: "backup/main".into(),
            path: "backups/gateway.ngbk".into(),
            token: "fixture-token".into(),
        };
        let downloaded = download_at(&destination, &base).await.unwrap();
        assert_eq!(downloaded.revision, sha);
        assert_eq!(downloaded.bytes, b"fixture-ciphertext");
        upload_at(&destination, downloaded.bytes, Some(&sha), &base)
            .await
            .unwrap();
        task.abort();
        let _ = task.await;
    }
}
