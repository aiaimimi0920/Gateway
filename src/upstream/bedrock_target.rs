//! Public-runtime allowlist for the new explicit auth modes; no automatic region fallback.
use crate::error::GatewayError;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::RequestPlan;

// AWS General Reference, Bedrock *runtime* table, checked 2026-11-02.
const REGIONS: &[&str] = &[
    "us-east-1",
    "us-east-2",
    "us-west-2",
    "ap-south-1",
    "ap-south-2",
    "ap-northeast-1",
    "ap-northeast-2",
    "ap-northeast-3",
    "ap-southeast-1",
    "ap-southeast-2",
    "ca-central-1",
    "eu-central-1",
    "eu-central-2",
    "eu-west-1",
    "eu-west-2",
    "eu-west-3",
    "eu-south-1",
    "eu-south-2",
    "eu-north-1",
    "sa-east-1",
    "us-gov-east-1",
    "us-gov-west-1",
];
const FIPS: &[&str] = &[
    "us-east-1",
    "us-east-2",
    "us-west-2",
    "ca-central-1",
    "us-gov-east-1",
    "us-gov-west-1",
];

fn invalid() -> GatewayError {
    GatewayError::bad_request(
        "Bedrock authentication target or region is not bound to an allowed runtime endpoint",
    )
    .with_code("bedrock_auth_target")
}

fn base_url(value: &str) -> Result<url::Url, GatewayError> {
    let url = url::Url::parse(value).map_err(|_| invalid())?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
    {
        return Err(invalid());
    }
    Ok(url)
}

fn host_region(host: &str) -> Option<String> {
    for (prefix, regions) in [
        ("bedrock-runtime.", REGIONS),
        ("bedrock-runtime-fips.", FIPS),
    ] {
        if let Some(region) = host
            .strip_prefix(prefix)
            .and_then(|tail| tail.strip_suffix(".amazonaws.com"))
        {
            if regions.contains(&region) {
                return Some(region.into());
            }
        }
    }
    None
}

pub(crate) fn validate_model_id(model: &str) -> Result<(), GatewayError> {
    // URI labels are encoded separately. ARN slashes are identity, never path separators.
    if model.is_empty()
        || model.len() > 2048
        || !model
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.:~/".contains(&byte))
        || model
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
        || (model.contains('/') && !model.starts_with("arn:"))
    {
        return Err(GatewayError::bad_request("Invalid Bedrock model identity")
            .with_code("bedrock_invalid_model_id"));
    }
    Ok(())
}

pub(super) fn validate(
    payload: &ProviderAccountPayload,
    plan: Option<&RequestPlan>,
    region: &str,
) -> Result<(), GatewayError> {
    let base = base_url(&payload.base_url)?;
    if host_region(base.host_str().ok_or_else(invalid)?).as_deref() != Some(region) {
        return Err(invalid());
    }
    // Configuration admission has no request yet; final send additionally binds the actual wire.
    let Some(plan) = plan else { return Ok(()) };
    let request = url::Url::parse(&plan.url).map_err(|_| invalid())?;
    if request.origin() != base.origin()
        || !request.username().is_empty()
        || request.password().is_some()
        || request.query().is_some()
        || request.fragment().is_some()
        || !plan.query.is_empty()
        || plan.method != rquest::Method::POST
    {
        return Err(invalid());
    }
    let path = request.path().strip_prefix("/model/").ok_or_else(invalid)?;
    let (label, stream) = if let Some(label) = path.strip_suffix("/converse-stream") {
        (label, true)
    } else {
        (path.strip_suffix("/converse").ok_or_else(invalid)?, false)
    };
    let model = percent_encoding::percent_decode_str(label)
        .decode_utf8()
        .map_err(|_| invalid())?;
    validate_model_id(&model)?;
    if crate::protocol::bedrock_converse::default_path(&model, stream) != request.path() {
        return Err(invalid());
    }
    Ok(())
}
