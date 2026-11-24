//! Parse either SAP BTP credential shape without allowing authentication-mode fallback.
use super::{config, invalid, Endpoint, GatewayError};
use serde::Deserialize;

pub struct ServiceKey {
    pub(super) clientid: String,
    pub(super) credential: Credential,
}

pub(super) enum Credential {
    Secret {
        url: String,
        secret: String,
    },
    Certificate {
        url: String,
        certificate: String,
        key: String,
    },
}

// Known fields deserialize directly so duplicate identity or credential fields are rejected.
#[derive(Deserialize)]
struct RawKey {
    clientid: String,
    clientsecret: Option<String>,
    url: Option<String>,
    #[serde(alias = "certUrl")]
    certurl: Option<String>,
    certificate: Option<String>,
    key: Option<String>,
    serviceurls: ServiceUrls,
}

#[derive(Deserialize)]
struct ServiceUrls {
    #[serde(rename = "AI_API_URL")]
    api: String,
}

impl ServiceKey {
    pub fn parse(raw: &str, endpoint: &Endpoint) -> Result<Self, GatewayError> {
        let (key, origin) = Self::parse_for_api(raw)?;
        if origin != endpoint.origin {
            return Err(invalid());
        }
        Ok(key)
    }

    pub(super) fn parse_for_api(raw: &str) -> Result<(Self, String), GatewayError> {
        if raw.len() > 65_536 {
            return Err(invalid());
        }
        let raw: RawKey = serde_json::from_str(raw).map_err(|_| invalid())?;
        if !secret_atom(&raw.clientid, 2048) {
            return Err(invalid());
        }
        let origin =
            config::api_origin(raw.serviceurls.api.trim_end_matches('/')).ok_or_else(invalid)?;
        let credential = match (raw.clientsecret, raw.certurl, raw.certificate, raw.key) {
            (Some(secret), None, None, None) if secret_atom(&secret, 16_384) => {
                let url = raw
                    .url
                    .as_deref()
                    .ok_or_else(invalid)?
                    .trim_end_matches('/');
                auth_site(url, false).ok_or_else(invalid)?;
                Credential::Secret {
                    url: url.into(),
                    secret,
                }
            }
            (None, Some(url), Some(certificate), Some(key)) => {
                let url = url.trim_end_matches('/');
                let site = auth_site(url, true).ok_or_else(invalid)?;
                if raw.url.as_ref().is_some_and(|v| {
                    auth_site(v.trim_end_matches('/'), false).as_ref() != Some(&site)
                }) {
                    return Err(invalid());
                }
                super::mtls::identity(&certificate, &key).map_err(|_| invalid())?;
                Credential::Certificate {
                    url: url.into(),
                    certificate,
                    key,
                }
            }
            _ => return Err(invalid()),
        };
        Ok((
            Self {
                clientid: raw.clientid,
                credential,
            },
            origin,
        ))
    }

    pub(super) fn secret_fragments(&self) -> Vec<&str> {
        match &self.credential {
            Credential::Secret { secret, .. } => vec![&self.clientid, secret],
            Credential::Certificate {
                certificate, key, ..
            } => vec![&self.clientid, certificate.trim(), key.trim()],
        }
    }
}

fn auth_site(value: &str, certificate: bool) -> Option<(String, String)> {
    let url = config::url(value)?;
    let host = url.host_str()?.strip_suffix(".hana.ondemand.com")?;
    let suffix = if certificate {
        ".authentication.cert."
    } else {
        ".authentication."
    };
    let (tenant, region) = host.split_once(suffix)?;
    (url.path() == "/" && config::atom(tenant, 200) && config::atom(region, 32))
        .then(|| (tenant.into(), region.into()))
}

fn secret_atom(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && value.bytes().all(|b| b.is_ascii_graphic())
}
