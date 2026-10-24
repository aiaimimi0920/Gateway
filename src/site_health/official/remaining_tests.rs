use super::{aliyun, azure, xai};
use serde_json::json;

fn azure_table(headers: &str, cells: &str) -> String {
    format!(
        r#"<table data-zone-name="americas" class="table region-status-table"><thead><tr><th>Products and services</th>{headers}</tr></thead><tr class="capability-row"><td>Azure OpenAI Service</td>{cells}</tr></table>"#
    )
}

fn cell(label: &str) -> String {
    format!(
        r#"<td class="status-cell"><span data-label="{label}"><svg><title>Not a region</title></svg></span><span class="hide-text">{label}</span></td>"#
    )
}

#[test]
fn azure_is_regional_not_global_and_not_applicable_is_not_an_outage() {
    for (label, expected) in [
        ("Good", "operational"),
        ("Not available", "not_available"),
        ("Information", "information"),
        ("Warning", "warning"),
        ("Critical", "major_outage"),
        ("Future", "unknown"),
    ] {
        let html = azure_table("<th><span>East US</span></th>", &cell(label));
        let rows = azure::parse(html.as_bytes()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Azure OpenAI / americas / East US");
        assert_eq!(rows[0].status, expected);
        assert!(rows[0].model.is_none());
    }
}

#[test]
fn azure_rejects_misaligned_columns_duplicate_regions_and_script_impostors() {
    let headers = "<th>East US</th><th>West US</th>";
    assert!(azure::parse(azure_table(headers, &cell("Good")).as_bytes()).is_none());
    assert!(azure::parse(
        azure_table("<th>East US</th>", &(cell("Good") + &cell("Good"))).as_bytes()
    )
    .is_none());
    assert!(azure::parse(
        azure_table(
            "<th>East US</th><th>East US</th>",
            &(cell("Good") + &cell("Good"))
        )
        .as_bytes()
    )
    .is_none());
    let good = azure_table("<th>East US</th>", &cell("Good"));
    assert!(azure::parse(
        good.replace("<th>East", "<th colspan=\"2\">East")
            .as_bytes()
    )
    .is_none());
    assert!(azure::parse(
        good.replace(
            "class=\"status-cell\"",
            "class=\"status-cell\" rowspan=\"2\""
        )
        .as_bytes()
    )
    .is_none());
    assert!(azure::parse(format!("{good}{good}").as_bytes()).is_none());
    assert!(azure::parse(format!("<script>{good}</script>").as_bytes())
        .unwrap()
        .is_empty());
    assert!(azure::parse(
        good.replace("Azure OpenAI Service", "Azure AI Search")
            .as_bytes()
    )
    .unwrap()
    .is_empty());
    assert!(azure::parse(good.replace("data-label", "changed-label").as_bytes()).is_none());
    assert!(azure::parse(&vec![b'x'; 8_388_609]).is_none());
    assert!(azure::parse(&[255]).is_none());
}

#[test]
fn xai_uses_api_product_not_grok_website_or_overall_status() {
    let mut body = json!({"overall":{"status":"available"},"products":[
        {"id":"grok","name":"Grok","status":"unavailable"},
        {"id":"api","name":"API","status":"available","components":[
            {"id":"api-global","name":"Global (api.x.ai)","status":"available"},
            {"id":"api-us","name":"US (us.api.x.ai)","status":"future"}]}]});
    let rows = xai::parse(&body).unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].status, "operational");
    assert_eq!(rows[2].status, "unknown");
    assert!(rows.iter().all(|o| o.model.is_none()));
    body["products"][1]["status"] = json!("future");
    assert_eq!(xai::parse(&body).unwrap()[0].status, "unknown");
    body["products"][1]["components"] = json!(null);
    assert!(xai::parse(&body).is_none());
    assert!(xai::parse(&json!({"products":[]})).unwrap().is_empty());
    assert!(xai::parse(&json!({"products":vec![json!({});65]})).is_none());
}

#[test]
fn aliyun_empty_notices_never_establish_qwen_health_and_business_denials_fail_closed() {
    let empty = json!({"code":200,"success":true,"data":[],"total":0});
    let rows = aliyun::parse(&empty).unwrap();
    assert_eq!(rows[0].status, "no_public_incidents");
    assert!(rows[0].model.is_none());
    assert!(aliyun::parse(&json!({"code":502,"success":false,"data":[],"total":0})).is_none());
    assert!(aliyun::parse(&json!({"code":200,"success":true,"data":[{}],"total":1})).is_none());
    let partial = json!({"code":200,"success":true,"data":[],"total":3});
    assert_eq!(
        aliyun::parse(&partial).unwrap()[0].status,
        "coverage_unknown"
    );
    let notices = json!({"code":200,"success":true,"data":vec![json!({"title":"Public event"});33],"total":33});
    let rows = aliyun::parse(&notices).unwrap();
    assert_eq!(rows.len(), 33); // Coverage row plus at most 32 notices.
    assert!(rows
        .iter()
        .all(|o| o.model.is_none() && o.status != "operational"));
}

#[test]
#[ignore = "Explicit local public-response replay; set GATEWAY_STATUS_FIXTURES to the evidence directory"]
fn captured_remaining_official_responses_replay() {
    let root = std::path::PathBuf::from(std::env::var("GATEWAY_STATUS_FIXTURES").unwrap());
    let azure = azure::parse(&std::fs::read(root.join("azure-direct.html")).unwrap()).unwrap();
    assert!(
        azure.len() >= 50,
        "full region matrix, not a single green sample"
    );
    assert!(azure
        .iter()
        .any(|o| o.name.contains("azure-china") && o.status == "not_available"));
    println!("Azure: {} regional observations", azure.len());
    for (adapter, file, count) in [
        (
            super::HealthAdapter::XaiStatus,
            "xai-api-status.remote.json",
            4,
        ),
        (
            super::HealthAdapter::YouStatus,
            "you-summary.remote.json",
            3,
        ),
        (super::HealthAdapter::TavilyStatus, "8.body", 1),
        (
            super::HealthAdapter::QwenStatus,
            "ali-cn-events-raw.json",
            1,
        ),
    ] {
        let body = serde_json::from_slice(&std::fs::read(root.join(file)).unwrap()).unwrap();
        let rows = super::parsing::parse(adapter, &body).unwrap();
        assert_eq!(rows.len(), count, "{}", adapter.id());
        println!("{}: {} observations", adapter.id(), rows.len());
    }
}

#[tokio::test]
async fn azure_html_size_exception_does_not_relax_other_response_limits() {
    use crate::site_health::{transport, MonitoringAuth};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let app = axum::Router::new()
        .route(
            "/large",
            axum::routing::get(|| async { vec![b'x'; 1_048_577] }),
        )
        .route(
            "/over",
            axum::routing::get(|| async { vec![b'x'; 8_388_609] }),
        );
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = rquest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(4))
        .build()
        .unwrap();
    let standard = transport::json(
        &client,
        &format!("{base}/large"),
        None,
        &MonitoringAuth::default(),
    )
    .await;
    let bootstrap = transport::bounded_body(client.get(format!("{base}/large"))).await;
    let regional = transport::regional_html(client.get(format!("{base}/large"))).await;
    let oversized = transport::regional_html(client.get(format!("{base}/over"))).await;
    server.abort();
    let _ = server.await;
    assert_eq!(standard.unwrap_err().0, "response_too_large");
    assert_eq!(bootstrap.unwrap_err().0, "response_too_large");
    assert_eq!(regional.unwrap().len(), 1_048_577);
    assert_eq!(oversized.unwrap_err().0, "response_too_large");
}
