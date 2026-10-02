//! Destructive semantics are opt-in by verified service contract, never by user assertion.
use crate::routing::config::CredentialStorageConnection;
use url::Url;

#[derive(Clone, Copy)]
pub(super) struct StorageCapabilities {
    pub conditional_create: bool,
    pub conditional_delete: bool,
}

pub(super) fn for_connection(connection: &CredentialStorageConnection) -> StorageCapabilities {
    match connection {
        CredentialStorageConnection::Local { .. } => StorageCapabilities {
            conditional_create: true,
            conditional_delete: true,
        },
        // RFC 4918 WebDAV uses HTTP's If-None-Match create-only precondition.
        // We do not infer deployment-specific atomic DELETE support from an ETag.
        CredentialStorageConnection::Webdav { .. } => StorageCapabilities {
            conditional_create: true,
            conditional_delete: false,
        },
        CredentialStorageConnection::S3 { endpoint, .. } => {
            let aws = official_aws_endpoint(endpoint);
            StorageCapabilities {
                conditional_create: aws || official_r2_endpoint(endpoint),
                conditional_delete: aws,
            }
        }
    }
}

fn secure_service_host(endpoint: &str) -> Option<String> {
    let url = Url::parse(endpoint).ok()?;
    if url.scheme() != "https"
        || url.port_or_known_default() != Some(443)
        || !matches!(url.path(), "" | "/")
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return None;
    }
    Some(url.host_str()?.to_string())
}

fn official_aws_endpoint(endpoint: &str) -> bool {
    let Some(host) = secure_service_host(endpoint) else {
        return false;
    };
    if host == "s3.amazonaws.com" {
        return true;
    }
    let Some(service) = host.strip_suffix(".amazonaws.com") else {
        return false;
    };
    let Some(region) = service
        .strip_prefix("s3.dualstack.")
        .or_else(|| service.strip_prefix("s3."))
    else {
        return false;
    };
    // Reviewed AWS endpoint table, 2026-10-02. Unknown/new regions and partitions
    // remain disabled until their service-specific conditional guarantees are reviewed.
    matches!(
        region,
        "us-east-1"
            | "us-east-2"
            | "us-west-1"
            | "us-west-2"
            | "af-south-1"
            | "ap-east-1"
            | "ap-east-2"
            | "ap-south-1"
            | "ap-south-2"
            | "ap-northeast-1"
            | "ap-northeast-2"
            | "ap-northeast-3"
            | "ap-southeast-1"
            | "ap-southeast-2"
            | "ap-southeast-3"
            | "ap-southeast-4"
            | "ap-southeast-5"
            | "ap-southeast-6"
            | "ap-southeast-7"
            | "ca-central-1"
            | "ca-west-1"
            | "eu-central-1"
            | "eu-central-2"
            | "eu-west-1"
            | "eu-west-2"
            | "eu-west-3"
            | "eu-south-1"
            | "eu-south-2"
            | "eu-north-1"
            | "il-central-1"
            | "mx-central-1"
            | "me-south-1"
            | "me-central-1"
            | "sa-east-1"
            | "us-gov-east-1"
            | "us-gov-west-1"
    )
}

fn official_r2_endpoint(endpoint: &str) -> bool {
    let Some(host) = secure_service_host(endpoint) else {
        return false;
    };
    let Some(account) = host.strip_suffix(".r2.cloudflarestorage.com") else {
        return false;
    };
    // Jurisdiction-specific endpoints require their own reviewed service contract.
    account.len() == 32 && account.bytes().all(|b| b.is_ascii_hexdigit())
}

pub(crate) fn archive_purge_support(
    connection: Option<&CredentialStorageConnection>,
) -> (bool, Option<&'static str>) {
    match connection {
        None | Some(CredentialStorageConnection::Local { .. }) => (true, None),
        Some(connection) if for_connection(connection).conditional_delete => (true, None),
        Some(CredentialStorageConnection::Webdav { .. }) => (false, Some("WebDAV archive purge is disabled because atomic conditional deletion has not been confirmed for this service")),
        Some(CredentialStorageConnection::S3 { .. }) => (false, Some("Archive purge is disabled for R2 and unverified S3-compatible services; atomic conditional deletion is only enabled for official AWS S3 HTTPS endpoints")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capability_requires_exact_official_https_service_not_suffix_or_http() {
        for value in [
            "https://s3.amazonaws.com",
            "https://s3.us-east-1.amazonaws.com",
            "https://s3.dualstack.eu-west-2.amazonaws.com",
        ] {
            assert!(official_aws_endpoint(value), "{value}");
        }
        for value in [
            "http://s3.amazonaws.com",
            "https://s3.amazonaws.com:8443",
            "https://s3.amazonaws.com/proxy",
            "https://s3.amazonaws.com.attacker.invalid",
            "https://bucket.s3.amazonaws.com",
            "https://s3-website-us-east-1.amazonaws.com",
            "https://s3.attacker.amazonaws.com",
            "https://s3.us-invented-1.amazonaws.com",
            "https://s3.us-east-1.amazonaws.com.cn",
            "https://s3.cn-north-1.amazonaws.com.cn",
            "https://s3.us-east-1.amazonaws.com?auth=value",
            "https://user@s3.amazonaws.com",
        ] {
            assert!(!official_aws_endpoint(value), "{value}");
        }
        assert!(official_r2_endpoint(
            "https://0123456789abcdef0123456789abcdef.r2.cloudflarestorage.com"
        ));
        assert!(!official_r2_endpoint(
            "https://0123456789abcdef0123456789abcdef.r2.cloudflarestorage.com.attacker.invalid"
        ));
        assert!(!official_r2_endpoint(
            "https://custom.r2.cloudflarestorage.com"
        ));
    }
}
