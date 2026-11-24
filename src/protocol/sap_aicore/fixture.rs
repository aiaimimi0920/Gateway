//! Process-owned SAP devtest transport: a fixed CONNECT peer and a separate direct mTLS peer.
use super::{service_key::Credential, GatewayError, ServiceKey};
use crate::upstream::client::sensenova_fixture::{
    load_isolated_with_root, loopback_proxy, namespaced_key,
};
use std::net::SocketAddrV4;

const PROXY: &str = "GATEWAY_DEVTEST_SAP_AICORE_PROXY";
const CA: &str = "GATEWAY_DEVTEST_SAP_AICORE_CA";
const MTLS: &str = "GATEWAY_DEVTEST_SAP_AICORE_MTLS";
const AZURE: &str = "https://api.ai.prod-eu20.westeurope.azure.ml.hana.ondemand.com";
const AWS: &str = "https://api.ai.prod.eu-central-1.aws.ml.hana.ondemand.com";
const AUTH: &str = "https://gateway-fixture.authentication.eu20.hana.ondemand.com";
const CERT_HOST: &str = "gateway-fixture.authentication.cert.eu20.hana.ondemand.com";
const CERT_AUTH: &str = "https://gateway-fixture.authentication.cert.eu20.hana.ondemand.com";
const CLIENT_PREFIX: &str = "gateway-sap-fixture-client-";
const SECRET_PREFIX: &str = "gateway-sap-fixture-secret-";

pub(super) struct Transport {
    pub(super) http: rquest::Client,
    mtls: SocketAddrV4,
    ca: reqwest::Certificate,
}

impl Transport {
    pub(super) fn load(config: &crate::config::Config) -> anyhow::Result<Option<Self>> {
        let mtls = std::env::var_os(MTLS);
        let proxy =
            load_isolated_with_root(config, "SAP AI Core", "packaged-sap-aicore-", PROXY, CA)?;
        match (proxy, mtls) {
            (None, None) => Ok(None),
            (Some(proxy), Some(mtls)) => {
                let mtls = mtls
                    .to_str()
                    .and_then(loopback_proxy)
                    .filter(|addr| addr.port() != config.port && addr.port() != 443)
                    .ok_or_else(|| anyhow::anyhow!("Invalid SAP AI Core fixture mTLS listener"))?;
                Ok(Some(Self {
                    http: proxy.http,
                    mtls,
                    ca: reqwest::Certificate::from_pem(&proxy.root_pem)?,
                }))
            }
            _ => {
                anyhow::bail!("SAP AI Core fixture requires both CONNECT and direct mTLS settings")
            }
        }
    }

    pub(super) fn mtls_target(
        &self,
        builder: reqwest::ClientBuilder,
        url: &str,
    ) -> Result<(reqwest::ClientBuilder, String), GatewayError> {
        if !url
            .strip_prefix(CERT_AUTH)
            .is_some_and(|path| path == "/oauth/token")
        {
            return Err(rejected());
        }
        // Keep SAP's hostname for SNI and certificate verification. Only the fixed
        // hostname gets a DNS override; no system trust, global resolver or proxy changes.
        let builder = builder
            .tls_built_in_root_certs(false)
            .add_root_certificate(self.ca.clone())
            .resolve(CERT_HOST, self.mtls.into());
        Ok((
            builder,
            format!("{CERT_AUTH}:{}/oauth/token", self.mtls.port()),
        ))
    }
}

fn rejected() -> GatewayError {
    GatewayError::bad_request(
        "SAP AI Core fixture requires an exact synthetic service key and origin.",
    )
    .with_code("sap_aicore_fixture_request_invalid")
}

pub(super) fn admit(key: &ServiceKey, origin: &str) -> Result<(), GatewayError> {
    let credential_matches = match &key.credential {
        Credential::Secret { url, secret } => url == AUTH && namespaced_key(secret, SECRET_PREFIX),
        Credential::Certificate { url, .. } => url == CERT_AUTH,
    };
    if matches!(origin, AWS | AZURE)
        && namespaced_key(&key.clientid, CLIENT_PREFIX)
        && credential_matches
    {
        Ok(())
    } else {
        Err(rejected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sap_packaged_fixture_admits_only_synthetic_identity_and_exact_origins() {
        let mut key = ServiceKey {
            clientid: format!("{CLIENT_PREFIX}{}", "a".repeat(32)),
            credential: Credential::Secret {
                url: AUTH.into(),
                secret: format!("{SECRET_PREFIX}{}", "b".repeat(32)),
            },
        };
        assert!(admit(&key, AZURE).is_ok());
        assert!(admit(&key, AWS).is_ok());
        assert!(admit(&key, "https://other.invalid").is_err());
        let Credential::Secret { url, .. } = &mut key.credential else {
            unreachable!()
        };
        url.push_str(".invalid");
        assert!(admit(&key, AZURE).is_err());
        let Credential::Secret { url, secret } = &mut key.credential else {
            unreachable!()
        };
        *url = AUTH.into();
        secret.push('a');
        assert!(admit(&key, AZURE).is_err());
        key.credential = Credential::Certificate {
            url: CERT_AUTH.into(),
            certificate: String::new(),
            key: String::new(),
        };
        // Production parsing verifies the PEM and identity before this fixture check.
        assert!(admit(&key, AWS).is_ok());
        key.clientid = "real-client".into();
        assert!(admit(&key, AWS).is_err());
    }
}
