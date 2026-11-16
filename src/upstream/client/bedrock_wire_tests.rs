//! Bounded loopback captures exercise both existing Bedrock execution paths.
use super::*;
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[path = "bedrock_wire_signature.rs"]
mod signature;

async fn receiver(response: Option<String>) -> (String, tokio::task::JoinHandle<Vec<u8>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let response = response.unwrap_or_else(|| {
        "HTTP/1.1 403 Forbidden\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}".into()
    });
    let task = tokio::spawn(async move {
        tokio::time::timeout(Duration::from_secs(8), async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut block = [0; 2048];
                let n = socket.read(&mut block).await.unwrap();
                assert!(n > 0 && bytes.len() + n <= 32768);
                bytes.extend_from_slice(&block[..n]);
                if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&bytes[..end]).to_ascii_lowercase();
                    let len = header
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length: "))
                        .unwrap()
                        .parse::<usize>()
                        .unwrap();
                    if bytes.len() == end + 4 + len {
                        break;
                    }
                }
            }
            // Failure is intentional: test authentication without fabricating a success envelope.
            socket.write_all(response.as_bytes()).await.unwrap();
            bytes
        })
        .await
        .expect("bounded receiver")
    });
    (base, task)
}

async fn exercise(streaming: bool) {
    for (aliases, model) in (0..3).flat_map(|aliases| {
        [
            "fixture",
            "arn:aws:bedrock:us-east-1:123456789012:inference-profile/us.fixture-v1:0",
        ]
        .map(|model| (aliases, model))
    }) {
        let (base, server) = receiver(None).await;
        // Legacy no-mode payload permits custom endpoints; explicit modes are AWS-bound separately.
        let extra = if aliases == 1 {
            json!({
                "aws_access_key_id":"AKIAFIXTURE", "aws_secret_access_key":"secret-fixture",
                "aws_session_token":"session-fixture", "aws_region":"us-east-1",
                "requestMetadata":{"fixture":"preserved"}
            })
        } else if aliases == 2 {
            json!({
                "AWS-Access-Key-Id":"AKIAFIXTURE", "AWS_SECRET_ACCESS_KEY":"secret-fixture",
                "AWS-Session-Token":"session-fixture", "REGION":"us-east-1",
                "requestMetadata":{"fixture":"preserved"}
            })
        } else {
            json!({
                "awsAccessKeyId":"AKIAFIXTURE", "awsSecretAccessKey":"secret-fixture",
                "awsSessionToken":"session-fixture", "awsRegion":"us-east-1",
                "requestMetadata":{"fixture":"preserved"}
            })
        };
        let payload: ProviderAccountPayload = serde_json::from_value(json!({
            "adapter":"bedrock_converse_compatible", "base_url":base, "api_key":"",
            "extra_body":extra
        }))
        .unwrap();
        let req = responses::normalize_responses(
            json!({"model":model,"input":"hello","stream":streaming}),
        )
        .unwrap();
        let mut client = UpstreamClient::new(5);
        client.http = rquest::Client::builder().no_proxy().build().unwrap();
        let result = tokio::time::timeout(Duration::from_secs(7), async {
            if streaming {
                client
                    .execute_stream(&payload, &req, model, None)
                    .await
                    .map(|_| ())
            } else {
                client
                    .execute(&payload, &req, model, None)
                    .await
                    .map(|_| ())
            }
        })
        .await
        .expect("bounded request");
        let captured = server.await.unwrap();
        assert!(result.is_err());
        let end = captured.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        let header = String::from_utf8_lossy(&captured[..end]).to_ascii_lowercase();
        assert!(
            header.contains("authorization: aws4-hmac-sha256 "),
            "stream={streaming}: {header}"
        );
        assert!(header.contains("x-amz-security-token: session-fixture"));
        let body: Value = serde_json::from_slice(&captured[end + 4..]).unwrap();
        assert_eq!(body["requestMetadata"]["fixture"], "preserved");
        for key in [
            "awsAccessKeyId",
            "awsSecretAccessKey",
            "awsSessionToken",
            "awsRegion",
            "aws_access_key_id",
            "aws_secret_access_key",
            "aws_session_token",
            "aws_region",
            "region",
            "AWS-Access-Key-Id",
            "AWS_SECRET_ACCESS_KEY",
            "AWS-Session-Token",
            "REGION",
        ] {
            assert!(
                body.get(key).is_none(),
                "private field sent: {key}, stream={streaming}"
            );
        }
        assert!(!String::from_utf8_lossy(&captured[end + 4..]).contains("secret-fixture"));
        signature::assert_captured_signature(
            &captured,
            &crate::protocol::bedrock_converse::default_path(model, true),
        );
    }
}

#[tokio::test]
async fn bedrock_buffered_signs_final_bytes_without_private_body_fields() {
    exercise(false).await;
}

#[tokio::test]
async fn bedrock_streaming_signs_final_bytes_without_private_body_fields() {
    exercise(true).await;
}

#[tokio::test]
async fn bedrock_final_sender_does_not_follow_redirect_with_sts_material() {
    let destination = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let response = format!("HTTP/1.1 307 Temporary Redirect\r\nLocation: http://{}/capture\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", destination.local_addr().unwrap());
    let (base, server) = receiver(Some(response)).await;
    let payload: ProviderAccountPayload = serde_json::from_value(json!({
        "adapter":"bedrock_converse_compatible", "base_url":base, "api_key":"",
        "extra_body":{"awsAccessKeyId":"AKIAFIXTURE", "awsSecretAccessKey":"secret-fixture", "awsSessionToken":"session-fixture", "awsRegion":"us-east-1"}
    })).unwrap();
    let request =
        responses::normalize_responses(json!({"model":"fixture", "input":"hello", "stream":true}))
            .unwrap();
    let mut client = UpstreamClient::new(1);
    // Deliberately leave the client's default redirect policy enabled.
    client.http = rquest::Client::builder().no_proxy().build().unwrap();
    let result = tokio::time::timeout(
        Duration::from_secs(3),
        client.execute_stream(&payload, &request, "fixture", None),
    )
    .await
    .unwrap();
    assert!(result.is_err());
    assert!(!server.await.unwrap().is_empty());
    assert!(
        tokio::time::timeout(Duration::from_millis(200), destination.accept())
            .await
            .is_err(),
        "redirect destination received credentials"
    );
}
