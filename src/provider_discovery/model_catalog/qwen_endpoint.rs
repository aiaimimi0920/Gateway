//! Ordinary Qwen workspace endpoints; never infer a product or region from an account ID.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QwenEndpoint {
    pub base_url: String,
    pub catalog_source: String,
}

impl QwenEndpoint {
    /// A descriptor is derived from the saved inference base, not an imported catalog URL.
    pub fn parse(base: &str) -> Option<Self> {
        let base = base.strip_suffix('/').unwrap_or(base);
        let host = base
            .strip_prefix("https://")?
            .strip_suffix("/compatible-mode/v1")?;
        let labels = host.strip_suffix(".maas.aliyuncs.com")?;
        let (workspace, region) = labels.split_once('.')?;
        if workspace.is_empty()
            || workspace.len() > 63
            || workspace == "trial"
            || !workspace
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            || !workspace.as_bytes().first()?.is_ascii_alphanumeric()
            || !workspace.as_bytes().last()?.is_ascii_alphanumeric()
        {
            return None;
        }
        let source_host = match region {
            "ap-southeast-1" => "dashscope-intl.aliyuncs.com",
            "cn-hongkong" => "cn-hongkong.dashscope.aliyuncs.com",
            "cn-beijing" | "ap-northeast-1" | "eu-central-1" | "us-east-1" => host,
            _ => return None,
        };
        Some(Self {
            base_url: base.to_owned(),
            catalog_source: format!("https://{source_host}/api/v1/models"),
        })
    }

    pub(super) fn matches_catalog(&self, source: &str, account_base: Option<&str>) -> bool {
        source == self.catalog_source && account_base == Some(self.base_url.as_str())
    }
}
