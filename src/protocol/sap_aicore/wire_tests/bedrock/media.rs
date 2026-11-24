//! Public Chat must put only opted-in inline bytes on the SAP Bedrock wire.
use super::*;

pub(in crate::protocol::sap_aicore::wire_tests) const PNG: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC";

// Generated from a synthetic 1x1 red bitmap.
const JPEG: &str = concat!(
    "/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAAgGBgcGBQgHBwcJCQgKDBQNDAsLDBkSEw8UHRofHh0aHBwgJC4nICIsIxwcKDcp",
    "LDAxNDQ0Hyc5PTgyPC4zNDL/2wBDAQkJCQwLDBgNDRgyIRwhMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIy",
    "MjIyMjIyMjIyMjIyMjL/wAARCAABAAEDASIAAhEBAxEB/8QAHwAAAQUBAQEBAQEAAAAAAAAAAAECAwQFBgcICQoL/8QAtRAA",
    "AgEDAwIEAwUFBAQAAAF9AQIDAAQRBRIhMUEGE1FhByJxFDKBkaEII0KxwRVS0fAkM2JyggkKFhcYGRolJicoKSo0NTY3ODk6",
    "Q0RFRkdISUpTVFVWV1hZWmNkZWZnaGlqc3R1dnd4eXqDhIWGh4iJipKTlJWWl5iZmqKjpKWmp6ipqrKztLW2t7i5usLDxMXG",
    "x8jJytLT1NXW19jZ2uHi4+Tl5ufo6erx8vP09fb3+Pn6/8QAHwEAAwEBAQEBAQEBAQAAAAAAAAECAwQFBgcICQoL/8QAtREA",
    "AgECBAQDBAcFBAQAAQJ3AAECAxEEBSExBhJBUQdhcRMiMoEIFEKRobHBCSMzUvAVYnLRChYkNOEl8RcYGRomJygpKjU2Nzg5",
    "OkNERUZHSElKU1RVVldYWVpjZGVmZ2hpanN0dXZ3eHl6goOEhYaHiImKkpOUlZaXmJmaoqOkpaanqKmqsrO0tba3uLm6wsPE",
    "xcbHyMnK0tPU1dbX2Nna4uPk5ebn6Onq8vP09fb3+Pn6/9oADAMBAAIRAxEAPwDi6KKK+ZP3E//Z",
);

async fn with_image_setting(peer: &peer::Peer, enabled: bool) -> Fixture {
    Fixture::configured(
        peer,
        false,
        crate::protocol::sap_aicore::test_support::BASE,
        KEY,
        json!({"sap_bedrock_converse":true,"sap_bedrock_image_input":enabled}),
    )
    .await
}

fn parts() -> Value {
    json!([{"type":"text","text":"describe"},
        {"type":"image_url","image_url":{"url":format!("data:image/png;base64,{PNG}"),"detail":"auto"}}])
}

#[tokio::test]
async fn sap_aicore_bedrock_inline_images_use_native_bytes_and_settle() {
    Box::pin(async {
        let peer = peer::Peer::raw_sequence(vec![reply(), events()]).await;
        let f = with_image_setting(&peer, true).await;
        crate::protocol::sap_aicore::test_support::origin(peer.origin.clone(), async {
            for streaming in [false, true] {
                let mut request = body("chat/completions", streaming);
                let mut content = parts();
                content.as_array_mut().unwrap().extend([
                    json!({"type":"text","text":"compare"}),
                    json!({"type":"image_url","image_url":{"url":format!("data:image/jpeg;base64,{JPEG}")}}),
                ]);
                request["messages"][0]["content"] = content;
                let response = f.open("chat/completions", request).await;
                let status = response.status();
                let text = drain(response.into_body()).await;
                assert_eq!(status, StatusCode::OK, "{text}");
                assert!(text.contains("合成"), "{text}");
                if streaming {
                    assert!(text.contains("[DONE]"), "{text}");
                } else {
                    assert!(text.contains("合成回复"), "{text}");
                }
            }
            for row in f.audits(2).await {
                assert_identity(&row);
                assert_eq!(row.status, "completed");
                assert_eq!(row.total_tokens, Some(5));
            }
            assert_eq!(f.remaining().await, 0);
        })
        .await;
        assert_eq!(peer.oauth.lock().unwrap().len(), 1);
        {
            let calls = peer.calls.lock().unwrap();
            assert_eq!(calls.len(), 2);
            for ((path, _, sent), operation) in calls.iter().zip(["converse", "converse-stream"]) {
                assert_eq!(
                    path,
                    &format!("/v2/inference/deployments/deployment1/{operation}")
                );
                assert_eq!(
                    sent["messages"],
                    json!([{"role":"user","content":[
                        {"text":"describe"},{"image":{"format":"png","source":{"bytes":PNG}}},
                        {"text":"compare"},{"image":{"format":"jpeg","source":{"bytes":JPEG}}}
                    ]}])
                );
                assert!(sent.get("sap_bedrock_image_input").is_none());
            }
        }
        f.finish().await;
        peer.stop().await;
    })
    .await;
}

#[tokio::test]
async fn sap_aicore_bedrock_media_rejects_unenabled_or_lossy_requests_before_oauth() {
    Box::pin(async {
        let peer = peer::Peer::raw(reply()).await;
        for setting in [None, Some(false), Some(true)] {
            let f = match setting {
                Some(enabled) => with_image_setting(&peer, enabled).await,
                None => fixture(&peer, false).await,
            };
            let mut cases = vec![];
            if setting == Some(true) {
                for url in [
                    "https://never-fetch.invalid/file.png",
                    "s3://bucket/file.png",
                    "data:image/webp;base64,YQ==",
                    "data:image/jpeg;base64,not-valid",
                ] {
                    let mut content = parts();
                    content[1]["image_url"]["url"] = json!(url);
                    cases.push(("user", content));
                }
                for detail in [json!("high"), json!("low"), json!(null)] {
                    let mut content = parts();
                    content[1]["image_url"]["detail"] = detail;
                    cases.push(("user", content));
                }
                let mut extended = parts();
                extended[1]["image_url"]["unexpected"] = json!(true);
                cases.push(("user", extended));
                cases.push(("assistant", parts()));
                cases.push(("user", json!(vec![parts()[1].clone(); 11])));
            } else {
                cases.push(("user", parts()));
            }
            crate::protocol::sap_aicore::test_support::origin(peer.origin.clone(), async {
                for (role, content) in cases {
                    let mut request = body("chat/completions", false);
                    request["messages"] =
                        json!([{"role":"user","content":"first"},{"role":role,"content":content}]);
                    let response = f.open("chat/completions", request).await;
                    let status = response.status();
                    let text = drain(response.into_body()).await;
                    assert_eq!(status, StatusCode::BAD_REQUEST, "{text}");
                    assert!(!text.contains(PNG));
                }
                assert_eq!(f.remaining().await, 2);
            })
            .await;
            f.finish().await;
        }
        assert!(peer.oauth.lock().unwrap().is_empty());
        assert!(peer.calls.lock().unwrap().is_empty());
        peer.stop().await;
    })
    .await;
}
