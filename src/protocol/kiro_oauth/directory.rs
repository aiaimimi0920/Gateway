//! Directory authorization Region is explicit and never inferred from a Kiro inference/profile URL.
use super::OAuthError;

// AWS General Reference IAM Identity Center OIDC, commercial partition, checked 2026-11-04.
const REGIONS: &[&str] = &[
    "us-east-2",
    "us-east-1",
    "us-west-1",
    "us-west-2",
    "af-south-1",
    "ap-east-1",
    "ap-south-2",
    "ap-southeast-3",
    "ap-southeast-5",
    "ap-southeast-4",
    "ap-south-1",
    "ap-southeast-6",
    "ap-northeast-3",
    "ap-northeast-2",
    "ap-southeast-1",
    "ap-southeast-2",
    "ap-east-2",
    "ap-southeast-7",
    "ap-northeast-1",
    "ca-central-1",
    "ca-west-1",
    "eu-central-1",
    "eu-west-1",
    "eu-west-2",
    "eu-south-1",
    "eu-west-3",
    "eu-south-2",
    "eu-north-1",
    "eu-central-2",
    "il-central-1",
    "mx-central-1",
    "me-south-1",
    "me-central-1",
    "sa-east-1",
];

pub(super) fn commercial_region(region: &str) -> bool {
    REGIONS.contains(&region)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Scope {
    #[serde(rename = "codewhisperer:analysis")]
    Analysis,
    #[serde(rename = "codewhisperer:completions")]
    Completions,
    #[serde(rename = "codewhisperer:conversations")]
    Conversations,
    #[serde(rename = "codewhisperer:taskassist")]
    TaskAssist,
    #[serde(rename = "codewhisperer:transformations")]
    Transformations,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Directory {
    region: String,
    start_url: String,
    builder_id: bool,
}
impl Directory {
    /// Historical official public Builder ID reference, not another application's client ID.
    pub fn builder_id() -> Self {
        Self {
            region: "us-east-1".into(),
            start_url: "https://view.awsapps.com/start".into(),
            builder_id: true,
        }
    }
    pub fn identity_center(region: &str, start_url: &str) -> Result<Self, OAuthError> {
        if !REGIONS.contains(&region) || !valid_start(start_url) {
            return Err(OAuthError::InvalidInput);
        }
        Ok(Self {
            region: region.into(),
            start_url: start_url.into(),
            builder_id: false,
        })
    }
    pub fn region(&self) -> &str {
        &self.region
    }
    pub fn start_url(&self) -> &str {
        &self.start_url
    }
    pub fn oidc_origin(&self) -> String {
        format!("https://oidc.{}.amazonaws.com", self.region)
    }

    // Trusted login destinations, not proof that a particular Builder ID deployment returns them.
    pub(super) fn verification_url(&self, value: &str) -> bool {
        let Some(url) = safe_https(value) else {
            return false;
        };
        let regional = [
            format!("device.sso.{}.amazonaws.com", self.region),
            format!("device.sso.{}.api.aws", self.region),
        ];
        if regional
            .iter()
            .any(|host| url.host_str() == Some(host.as_str()))
        {
            return url.path() == "/";
        }
        self.builder_id
            && url.host_str() == Some("view.awsapps.com")
            && matches!(url.path(), "/start" | "/start/")
    }
}
fn safe_https(value: &str) -> Option<url::Url> {
    if value.is_empty() || value.len() > 4096 || value.bytes().any(|b| !(0x21..=0x7e).contains(&b))
    {
        return None;
    }
    let url = url::Url::parse(value).ok()?;
    (url.scheme() == "https"
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none())
    .then_some(url)
}
fn valid_start(value: &str) -> bool {
    let Some(url) = safe_https(value) else {
        return false;
    };
    let Some(label) = url
        .host_str()
        .and_then(|host| host.strip_suffix(".awsapps.com"))
    else {
        return false;
    };
    !label.is_empty()
        && label.len() <= 63
        && label
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !label.starts_with('-')
        && !label.ends_with('-')
        && matches!(url.path(), "/start" | "/start/")
        && url.query().is_none()
        && url.fragment().is_none()
}
