//! Read existing deployments in one resource group. No inference or configuration writes.
use super::{
    auth::exchange_token, config::atom, failure, invalid, Endpoint, GatewayError, ServiceKey,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, time::Duration};

const PAGE_SIZE: usize = 50;
const MAX_PAGES: usize = 5;
const MAX_PAGE_BYTES: usize = 512 * 1024;
const BUDGET: Duration = Duration::from_secs(15);
const STATUS: &str = "RUNNING";

#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DeploymentProtocol {
    #[default]
    Azure,
    Bedrock,
    Orchestration,
}

impl DeploymentProtocol {
    fn scenario(self) -> &'static str {
        match self {
            Self::Azure | Self::Bedrock => "foundation-models",
            Self::Orchestration => "orchestration",
        }
    }

    fn executable(self) -> Option<&'static str> {
        match self {
            Self::Azure => Some("azure-openai"),
            Self::Bedrock => Some("aws-bedrock"),
            // SAP's Orchestration SDK resolves by scenario and running status only.
            Self::Orchestration => None,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeploymentCatalog {
    deployments: Vec<Deployment>,
    truncated: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Deployment {
    id: String,
    deployment_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    configuration_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model_version: Option<String>,
}

#[derive(Deserialize)]
struct Page {
    resources: Vec<serde_json::Value>,
    count: Option<usize>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    id: String,
    deployment_url: String,
    scenario_id: String,
    executable_id: Option<String>,
    status: String,
    configuration_name: Option<serde_json::Value>,
    details: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct Details {
    resources: Resources,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Resources {
    backend_details: BackendDetails,
}
#[derive(Deserialize)]
struct BackendDetails {
    model: Model,
}
#[derive(Deserialize)]
struct Model {
    name: String,
    version: Option<String>,
}

impl Entry {
    fn into_choice(self, origin: &str, protocol: DeploymentProtocol) -> Option<Deployment> {
        let endpoint = Endpoint::parse(&self.deployment_url)?;
        if endpoint.origin != origin
            || endpoint.deployment != self.id
            || self.scenario_id != protocol.scenario()
            || protocol
                .executable()
                .is_some_and(|id| self.executable_id.as_deref() != Some(id))
            || self.status != STATUS
        {
            return None;
        }
        let model = self
            .details
            .and_then(|details| serde_json::from_value::<Details>(details).ok())
            .map(|details| details.resources.backend_details.model);
        let (model, configuration_name) = match protocol {
            DeploymentProtocol::Azure => {
                // Azure shares this executable with embeddings; keep the existing Chat allowlist.
                let model = model.filter(|model| {
                    matches!(
                        model.name.as_str(),
                        "gpt-4o" | "gpt-4o-mini" | "gpt-4.1" | "gpt-4.1-mini" | "gpt-4.1-nano"
                    )
                })?;
                (Some(model), None)
            }
            DeploymentProtocol::Bedrock | DeploymentProtocol::Orchestration => {
                // Directory metadata only labels candidates; it neither proves Converse support
                // nor guarantees that the SDK's model details are present for this deployment.
                let name = self
                    .configuration_name
                    .as_ref()
                    .and_then(|value| value.as_str())
                    .filter(|value| valid_label(value, 256))
                    .map(str::to_owned);
                let model = match protocol {
                    DeploymentProtocol::Bedrock => {
                        model.filter(|model| valid_label(&model.name, 128))
                    }
                    // A workflow deployment is not a declaration of its selectable models.
                    _ => None,
                };
                (model, name)
            }
        };
        let (model_name, model_version) = match model {
            Some(model) => (Some(model.name), model.version),
            None => (None, None),
        };
        Some(Deployment {
            id: self.id,
            deployment_url: self.deployment_url,
            configuration_name,
            model_name,
            model_version: model_version.filter(|value| {
                !value.is_empty()
                    && value.len() <= 128
                    && value
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
            }),
        })
    }
}

fn valid_label(value: &str, limit: usize) -> bool {
    !value.trim().is_empty() && value.len() <= limit && !value.chars().any(char::is_control)
}

pub(crate) async fn discover(
    http: &rquest::Client,
    auth: &super::auth::TokenCache,
    service_key: &str,
    resource_group: &str,
    protocol: DeploymentProtocol,
) -> Result<DeploymentCatalog, GatewayError> {
    if !atom(resource_group, 253) {
        return Err(invalid());
    }
    let (key, origin) = ServiceKey::parse_for_api(service_key)?;
    if auth.fixture.is_some() {
        super::fixture::admit(&key, &origin)?;
    }
    let http = auth.http(http);
    // This temporary lookup never touches the inference token cache or the credential store.
    tokio::time::timeout(BUDGET, async {
        let token = exchange_token(http, &key, auth.fixture.as_ref()).await?;
        drop(key);
        read_pages(http, &origin, resource_group, &token.value, protocol).await
    })
    .await
    .map_err(|_| failure("sap_aicore_deployments_timeout"))?
}

async fn read_pages(
    http: &rquest::Client,
    origin: &str,
    resource_group: &str,
    token: &str,
    protocol: DeploymentProtocol,
) -> Result<DeploymentCatalog, GatewayError> {
    let url = format!("{origin}/v2/lm/deployments");
    #[cfg(test)]
    let url = super::test_support::url(&url);
    let mut deployments = BTreeMap::new();
    let mut skip = 0;
    let mut truncated = true;
    let top = PAGE_SIZE.to_string();
    for _ in 0..MAX_PAGES {
        let offset = skip.to_string();
        let mut query = vec![
            ("scenarioId", protocol.scenario()),
            ("status", STATUS),
            ("$top", top.as_str()),
            ("$skip", offset.as_str()),
        ];
        if let Some(executable) = protocol.executable() {
            query.push(("executableIds", executable));
        }
        let response = http
            .get(&url)
            .default_headers(false)
            .redirect(rquest::redirect::Policy::none())
            .bearer_auth(token)
            .header("Accept", "application/json")
            .header("AI-Resource-Group", resource_group)
            // rquest replaces the previous query when query() is called again.
            .query(&query)
            .send()
            .await
            .map_err(|_| list_failed())?;
        if !response.status().is_success() {
            return Err(list_failed());
        }
        let bytes = crate::protocol::upstream_body::collect_bounded_upstream_stream_with_limit_and_provider(
            response.bytes_stream(), "SAP deployments", MAX_PAGE_BYTES, None,
        ).await.map_err(|_| list_failed())?;
        let Page { resources, count } =
            serde_json::from_slice(&bytes).map_err(|_| list_failed())?;
        let len = resources.len();
        if len > PAGE_SIZE {
            return Err(list_failed());
        }
        for value in resources {
            if let Some(choice) = serde_json::from_value::<Entry>(value)
                .ok()
                .and_then(|entry| entry.into_choice(origin, protocol))
            {
                deployments.entry(choice.id.clone()).or_insert(choice);
            }
        }
        // Advance by raw records, never by the number surviving the local protocol filter.
        skip += len;
        if len < PAGE_SIZE && count.is_none_or(|total| total <= skip) {
            truncated = false;
            break;
        }
        if len == 0 {
            break;
        }
    }
    Ok(DeploymentCatalog {
        deployments: deployments.into_values().collect(),
        truncated,
    })
}

fn list_failed() -> GatewayError {
    failure("sap_aicore_deployments_failed")
}
