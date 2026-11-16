//! Test-only destination substitution after production validation. Missing scopes never reach SAP.
tokio::task_local! { static ORIGIN: String; }
pub(crate) const BASE: &str = "https://api.ai.prod.eu-central-1.aws.ml.hana.ondemand.com/v2/inference/deployments/deployment1";
pub(crate) const KEY: &str = r#"{"clientid":"synthetic-sap-client","clientsecret":"synthetic-sap-secret","url":"https://synthetic.authentication.eu10.hana.ondemand.com","serviceurls":{"AI_API_URL":"https://api.ai.prod.eu-central-1.aws.ml.hana.ondemand.com"}}"#;
pub(crate) async fn origin<T>(origin: String, task: impl std::future::Future<Output = T>) -> T {
    ORIGIN.scope(origin, task).await
}
pub(super) fn url(original: &str) -> String {
    let parsed = url::Url::parse(original).unwrap();
    ORIGIN
        .try_with(|origin| format!("{origin}{}", parsed.path()))
        .expect("SAP test transport scope required")
}
