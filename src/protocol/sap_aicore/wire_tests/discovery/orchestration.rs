//! Only running Orchestration rows from the selected AI Core origin become choices.
use super::*;

pub(super) fn catalog() -> Value {
    let entry = |id: &str| {
        let mut row = deployment(id, "gpt-4o");
        row["scenarioId"] = json!("orchestration");
        row.as_object_mut().unwrap().remove("executableId");
        row
    };
    let mut labeled = entry("orch");
    labeled["configurationName"] = json!("workflow-a");
    let mut stopped = entry("stopped");
    stopped["status"] = json!("STOPPED");
    let mut moved = entry("foreign");
    moved["deploymentUrl"] = json!(format!(
        "{}/v2/inference/deployments/foreign",
        crate::protocol::sap_aicore::test_support::AZURE_ORIGIN
    ));
    let mut wrong_id = entry("mismatch");
    wrong_id["deploymentUrl"] = labeled["deploymentUrl"].clone();
    json!({"resources":[labeled, stopped, moved, wrong_id,
        deployment("azure", "gpt-4o"), bedrock_deployment("bedrock")]})
}

#[tokio::test]
async fn sap_aicore_orchestration_discovery_only_lists_bound_running_workflows() {
    let peer = DiscoveryPeer::new("orchestration").await;
    let fixture = peer.fixture();
    let revision = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_owned();
    let request = json!({"serviceKey":KEY,"resourceGroup":GROUP,"protocol":"orchestration"});
    let denied = send_console_json(
        &fixture.state,
        Method::POST,
        PATH,
        request.clone(),
        None,
        None,
    )
    .await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    assert!(peer.state.calls.lock().unwrap().is_empty());
    let grant = grant_console_secret_access(&fixture.state).await;
    let response = crate::protocol::sap_aicore::test_support::origin(
        peer.origin.clone(),
        send_console_json(
            &fixture.state,
            Method::POST,
            PATH,
            request,
            Some(&grant),
            None,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(
        parse_json(response).await,
        json!({"deployments":[{"id":"orch",
        "deploymentUrl":format!("{ORIGIN}/v2/inference/deployments/orch"),
        "configurationName":"workflow-a"}],"truncated":false})
    );
    assert_eq!(
        *peer.state.calls.lock().unwrap(),
        ["/oauth/token", "/v2/lm/deployments"]
    );
    let snapshot = fixture.state.route_config.snapshot();
    assert_eq!(snapshot.revision().id(), revision);
    assert!(snapshot.document().providers.is_empty());
}
