use super::{execute, test_support::*};
use crate::{error::ErrorKind, protocol::canonical::EndpointKind};
use serde_json::json;
use std::time::Duration;

#[tokio::test]
async fn luma_agents_video_submit_poll_and_container_read_preserve_actual_credit_zero() {
    let mut queued = job("queued");
    queued["model"] = json!("ray-3.2");
    queued["type"] = json!("video");
    let mut completed = queued.clone();
    completed["state"] = json!("completed");
    completed["output"] = json!([{"type":"video","url":ASSET}]);
    completed["usage"] = json!({"credits_charged":0});
    let server = Server::start(vec![
        Reply::json(201, queued),
        Reply::json(200, completed),
        Reply::mp4(),
    ])
    .await;
    let mut req = request(
        json!({"model":"alias","prompt":"video","response_format":"b64_json",
        "video":{"resolution":"720p","duration":"5s","loop":false}}),
    );
    req.endpoint_kind = EndpointKind::VideosGenerations;
    let output = within(
        &server.origin,
        execute(
            &server.client(),
            Duration::from_secs(5),
            &payload(),
            &req,
            "ray-3.2",
        ),
    )
    .await
    .unwrap();
    assert_eq!(output["object"], "video.generation");
    assert_eq!(output["completed"], true);
    assert!(!output["data"][0]["body_base64"]
        .as_str()
        .unwrap()
        .is_empty());
    assert_eq!(
        output["gateway_luma_generation"]["usage"]["credits_charged"],
        0
    );
    assert_eq!(server.calls()[0].body["video"], req.raw_body["video"]);
    assert_eq!(
        server.calls().iter().filter(|c| c.method == "POST").count(),
        1
    );
}

#[tokio::test]
async fn luma_agents_dispatcher_ignores_caller_and_default_auth_headers() {
    let mut completed = job("completed");
    completed["usage"] = json!({"credits_charged":0.25});
    let server = Server::start(vec![Reply::json(201, completed), Reply::png()]).await;
    let mut upstream = crate::upstream::client::UpstreamClient::new(5);
    upstream.http = server.client();
    let mut payload = payload();
    payload
        .headers
        .insert("Cookie".into(), "private-default".into());
    payload
        .headers
        .insert("Authorization".into(), "Bearer wrong-product".into());
    let extra = std::collections::HashMap::from([
        ("cookie".into(), "caller-cookie".into()),
        ("authorization".into(), "caller-key".into()),
    ]);
    let output = within(
        &server.origin,
        upstream.execute_json_passthrough(
            "luma-api-account",
            crate::routing::candidate::ProviderExecutionMode::DirectHttp,
            &payload,
            &request(json!({"prompt":"image"})),
            "uni-1",
            Some(&extra),
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        output["gateway_luma_generation"]["usage"]["credits_charged"],
        0.25
    );
    assert_eq!(server.calls().len(), 2);
    assert_eq!(
        server.calls()[0].headers["authorization"],
        "Bearer luma-api-synthetic-only"
    );
    assert!(!server.calls()[0].headers.contains_key("cookie"));
    assert!(!server.calls()[1].headers.contains_key("authorization"));
}
#[tokio::test]
async fn luma_agents_wrong_job_model_type_state_and_id_never_download() {
    for (key, value) in [
        ("id", json!("../redirect")),
        ("model", json!("uni-1-max")),
        ("type", json!("video")),
        ("state", json!("unknown")),
        ("output", json!([])),
        ("usage", json!({"credits_charged":-1})),
    ] {
        let mut body = job("completed");
        body[key] = value;
        let server = Server::start(vec![Reply::json(201, body)]).await;
        let error = within(
            &server.origin,
            execute(
                &server.client(),
                Duration::from_secs(5),
                &payload(),
                &request(json!({"prompt":"image"})),
                "uni-1",
            ),
        )
        .await
        .unwrap_err();
        assert_eq!(
            error.code.as_deref(),
            Some("luma_agents_generation"),
            "{key}"
        );
        assert_eq!(server.calls().len(), 1);
    }
    let mut wrong = job("completed");
    wrong["id"] = json!("00000000-0000-0000-0000-000000000001");
    let server = Server::start(vec![
        Reply::json(201, job("queued")),
        Reply::json(200, wrong),
    ])
    .await;
    assert!(within(
        &server.origin,
        execute(
            &server.client(),
            Duration::from_secs(5),
            &payload(),
            &request(json!({"prompt":"image"})),
            "uni-1"
        )
    )
    .await
    .is_err());
    assert_eq!(server.calls().len(), 2);
}
#[tokio::test]
async fn luma_agents_http_errors_redirects_and_unknown_start_do_not_retry() {
    for status in [200, 204, 301, 400, 401, 403, 429, 500, 503] {
        let server = Server::start(vec![Reply::json(
            status,
            json!({"detail":"private key luma-api-secret","retry":true}),
        )])
        .await;
        let error = within(
            &server.origin,
            execute(
                &server.client(),
                Duration::from_secs(5),
                &payload(),
                &request(json!({"prompt":"image"})),
                "uni-1",
            ),
        )
        .await
        .unwrap_err();
        assert_eq!(
            error.http_status,
            Some(if status < 400 { 502 } else { status })
        );
        assert!(!error.retryable);
        assert!(!error.message.contains("private"));
        assert_eq!(server.calls().len(), 1);
    }
}
#[tokio::test]
async fn luma_agents_json_limit_truncation_and_corrupt_images_are_rejected() {
    for reply in [
        Reply {
            status: 201,
            content_type: "application/json",
            bytes: vec![b' '; 65537],
            hold: false,
        },
        Reply {
            status: 201,
            content_type: "application/json",
            bytes: b"{\"id\":".to_vec(),
            hold: false,
        },
        Reply {
            status: 201,
            content_type: "text/html",
            bytes: b"login".to_vec(),
            hold: false,
        },
    ] {
        let server = Server::start(vec![reply]).await;
        assert!(within(
            &server.origin,
            execute(
                &server.client(),
                Duration::from_secs(5),
                &payload(),
                &request(json!({"prompt":"image"})),
                "uni-1"
            )
        )
        .await
        .is_err());
        assert_eq!(server.calls().len(), 1);
    }
    let mut png = Reply::png();
    png.bytes.truncate(40);
    let server = Server::start(vec![Reply::json(201, job("completed")), png]).await;
    let error = within(
        &server.origin,
        execute(
            &server.client(),
            Duration::from_secs(5),
            &payload(),
            &request(json!({"prompt":"image"})),
            "uni-1",
        ),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("luma_agents_asset_type"));
}
#[tokio::test]
async fn luma_agents_edit_wire_and_caller_drop_never_create_a_second_job() {
    let mut edited = job("completed");
    edited["type"] = json!("image_edit");
    let server = Server::start(vec![Reply::json(201, edited), Reply::png()]).await;
    let mut req = request(json!({"prompt":"edit","source":{"generation_id":ID}}));
    req.endpoint_kind = EndpointKind::ImagesEdits;
    within(
        &server.origin,
        execute(
            &server.client(),
            Duration::from_secs(5),
            &payload(),
            &req,
            "uni-1",
        ),
    )
    .await
    .unwrap();
    assert_eq!(server.calls()[0].body["type"], "image_edit");
    assert_eq!(server.calls()[0].body["source"]["generation_id"], ID);
    let server = Server::start(vec![Reply {
        hold: true,
        ..Reply::json(201, job("queued"))
    }])
    .await;
    let origin = server.origin.clone();
    let client = server.client();
    let task = tokio::spawn(async move {
        within(
            &origin,
            execute(
                &client,
                Duration::from_secs(5),
                &payload(),
                &request(json!({"prompt":"image"})),
                "uni-1",
            ),
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        while server.calls().is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    server.wait_closed().await;
    assert_eq!(server.calls().len(), 1);
}

#[tokio::test]
async fn luma_agents_cumulative_metadata_budget_and_invalid_asset_status_are_not_success() {
    let mut queued = job("queued");
    queued["padding"] = json!("x".repeat(60000));
    let replies = std::iter::once(Reply::json(201, queued.clone()))
        .chain((0..72).map(|_| Reply::json(200, queued.clone())))
        .collect();
    let server = Server::start(replies).await;
    let error = within(
        &server.origin,
        execute(
            &server.client(),
            Duration::from_secs(5),
            &payload(),
            &request(json!({"prompt":"image"})),
            "uni-1",
        ),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("luma_agents_body_limit"));
    assert!(server.calls().len() < 73);
    assert_eq!(
        server.calls().iter().filter(|c| c.method == "POST").count(),
        1
    );
    for status in [201, 204, 302, 403] {
        let mut asset = Reply::png();
        asset.status = status;
        let server = Server::start(vec![Reply::json(201, job("completed")), asset]).await;
        let error = within(
            &server.origin,
            execute(
                &server.client(),
                Duration::from_secs(5),
                &payload(),
                &request(json!({"prompt":"image"})),
                "uni-1",
            ),
        )
        .await
        .unwrap_err();
        assert_eq!(error.code.as_deref(), Some("luma_agents_asset_http"));
        assert_eq!(
            error.http_status,
            Some(if status < 400 { 502 } else { status })
        );
        assert_eq!(server.calls().len(), 2);
    }
}
#[tokio::test]
async fn luma_agents_bad_material_or_request_never_reaches_http() {
    let server = Server::start(vec![]).await;
    let mut material = payload();
    material.api_key = "wos-session=not-an-api-key".into();
    let error = within(
        &server.origin,
        execute(
            &server.client(),
            Duration::from_secs(1),
            &material,
            &request(json!({"prompt":"image"})),
            "uni-1",
        ),
    )
    .await
    .unwrap_err();
    assert_eq!(error.kind, ErrorKind::BadRequest);
    assert!(within(
        &server.origin,
        execute(
            &server.client(),
            Duration::from_secs(1),
            &payload(),
            &request(json!({"prompt":"image","n":2})),
            "uni-1"
        )
    )
    .await
    .is_err());
    assert!(server.calls().is_empty());
}
