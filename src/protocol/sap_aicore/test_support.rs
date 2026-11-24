//! Test-only destination substitution after production validation. Missing scopes never reach SAP.
tokio::task_local! { static ORIGIN: String; }
pub(crate) const BASE: &str = "https://api.ai.prod.eu-central-1.aws.ml.hana.ondemand.com/v2/inference/deployments/deployment1";
pub(crate) const KEY: &str = r#"{"clientid":"synthetic-sap-client","clientsecret":"synthetic-sap-secret","url":"https://synthetic.authentication.eu10.hana.ondemand.com","serviceurls":{"AI_API_URL":"https://api.ai.prod.eu-central-1.aws.ml.hana.ondemand.com"}}"#;
pub(super) const AZURE_ORIGIN: &str =
    "https://api.ai.prod-eu20.westeurope.azure.ml.hana.ondemand.com";
pub(super) fn azure_base() -> String {
    format!("{AZURE_ORIGIN}/v2/inference/deployments/deployment1")
}
pub(super) fn azure_key() -> String {
    let mut key: serde_json::Value = serde_json::from_str(KEY).unwrap();
    key["serviceurls"]["AI_API_URL"] = serde_json::json!(format!("{AZURE_ORIGIN}/v2"));
    key["url"] = serde_json::json!("https://synthetic.authentication.eu20.hana.ondemand.com");
    key.to_string()
}
pub(crate) async fn origin<T>(origin: String, task: impl std::future::Future<Output = T>) -> T {
    ORIGIN.scope(origin, task).await
}

pub(crate) fn inherit<T>(
    task: impl std::future::Future<Output = T>,
) -> impl std::future::Future<Output = T> {
    let origin = ORIGIN.try_with(Clone::clone).ok();
    let mtls = MTLS_TARGET.try_with(Clone::clone).ok();
    async move {
        let task = async move {
            match mtls {
                Some(target) => MTLS_TARGET.scope(target, task).await,
                None => task.await,
            }
        };
        match origin {
            Some(value) => ORIGIN.scope(value, task).await,
            None => task.await,
        }
    }
}
pub(super) fn url(original: &str) -> String {
    let parsed = url::Url::parse(original).unwrap();
    ORIGIN
        .try_with(|origin| format!("{origin}{}", parsed.path()))
        .expect("SAP test transport scope required")
}

// Certificate exchange has its own listener and trust anchor; the AI API stays on ORIGIN.
tokio::task_local! { static MTLS_TARGET: (String, reqwest::Certificate); }

pub(super) async fn mtls<T>(
    origin: String,
    ca: &[u8],
    task: impl std::future::Future<Output = T>,
) -> T {
    MTLS_TARGET
        .scope((origin, reqwest::Certificate::from_pem(ca).unwrap()), task)
        .await
}

pub(super) fn mtls_target(
    builder: reqwest::ClientBuilder,
    original: &str,
) -> (reqwest::ClientBuilder, String) {
    let path = url::Url::parse(original).unwrap();
    MTLS_TARGET
        .try_with(|(origin, ca)| {
            (
                builder
                    .tls_built_in_root_certs(false)
                    .add_root_certificate(ca.clone()),
                format!("{origin}{}", path.path()),
            )
        })
        .expect("SAP mTLS test transport scope required")
}
