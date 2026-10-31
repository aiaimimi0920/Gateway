//! Synthetic accepted-turn recovery; no credentials, storage or browser are used.
use super::*;
use axum::{
    extract::{Form, Query},
    routing::get,
};
use std::collections::HashMap;

#[path = "image_recovery_cookie_tests.rs"]
mod cookies;
#[path = "image_recovery_failure_tests.rs"]
mod failure;
#[path = "image_generation_cookie_tests.rs"]
mod generation_cookies;

fn candidate(id: &str, final_image: bool) -> Value {
    let mut value = vec![Value::Null; 13];
    value[0] = json!(id);
    value[8] = json!([if final_image { 2 } else { 1 }]);
    if final_image {
        value[12] = json!([
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            [[[[
                null,
                null,
                null,
                [
                    null,
                    null,
                    "fixture",
                    "https://lh3.googleusercontent.com/fixture.png"
                ]
            ]]]]
        ]);
    }
    json!(value)
}

fn accepted() -> String {
    let body = json!([[
        "wrb.fr",
        null,
        json!([
            null,
            ["c_fixture", "r_fixture"],
            null,
            null,
            [candidate("rc_fixture", false)]
        ])
        .to_string()
    ]])
    .to_string();
    format!(")]}}'\n{}\n{}\n", body.encode_utf16().count(), body)
}

fn history(final_image: bool) -> String {
    let turns = json!([[
        ["c_fixture", "r_fixture"],
        null,
        null,
        [[candidate("rc_fixture", final_image)]]
    ]]);
    json!([[
        "wrb.fr",
        "hNvQHb",
        json!([turns]).to_string(),
        null,
        null,
        null,
        "generic"
    ]])
    .to_string()
}

#[tokio::test]
async fn accepted_image_recovers_same_candidate_without_regeneration() {
    let generations = Arc::new(AtomicUsize::new(0));
    let reads = Arc::new(AtomicUsize::new(0));
    let gen = generations.clone();
    let read = reads.clone();
    let app = Router::new()
        .route(gemini_web::GEMINI_WEB_DEFAULT_STREAM_GENERATE_PATH,post(move || {
            gen.fetch_add(1,Ordering::SeqCst);
            async { accepted() }
        }))
        .route("/app",get(|| async { r#"<script>window.WIZ_global_data={"SNlM0e":"fixture-xsrf","cfb2h":"fixture-build","FdrFJe":"fixture-session"};</script>"# }))
        .route("/_/BardChatUi/data/batchexecute",post(move |Query(query):Query<HashMap<String,String>>,Form(form):Form<HashMap<String,String>>| {
            let count = read.fetch_add(1,Ordering::SeqCst);
            async move {
                assert_eq!(query.get("rpcids").map(String::as_str),Some("hNvQHb"));
                let rpc:Value=serde_json::from_str(&form["f.req"]).unwrap();
                let args:Value=serde_json::from_str(rpc[0][0][1].as_str().unwrap()).unwrap();
                assert_eq!(args,json!(["c_fixture",10,null,1,[1],[4],null,1]));
                assert_eq!(form["at"],"fixture-xsrf");
                history(count > 0)
            }
        }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let prepared = prepared(&base);
    let mut client = UpstreamClient::new(1);
    client.http = rquest::Client::builder().no_proxy().build().unwrap();
    let body = client
        .send_gemini_web_image_template_once(
            &payload(&base),
            &prepared,
            prepared.replay_template.as_ref().unwrap(),
        )
        .await
        .unwrap();
    let result = client
        .resolve_gemini_web_image_assets(
            &payload(&base),
            &prepared.session,
            body.cookies,
            &body.text,
            Duration::from_secs(10),
        )
        .await;
    server.abort();
    let _ = server.await;
    assert!(
        result.is_ok(),
        "accepted image must be recovered by a read, got {:?}",
        result.err()
    );
    assert_eq!(generations.load(Ordering::SeqCst), 1);
    assert_eq!(reads.load(Ordering::SeqCst), 2);
}
