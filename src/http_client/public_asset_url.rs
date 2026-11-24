//! Shared URL syntax boundary for media links; downloaders separately pin public DNS results.
use std::net::IpAddr;

pub(crate) fn checked(raw: &str) -> Result<url::Url, ()> {
    let url = url::Url::parse(raw).map_err(|_| ())?;
    if raw.len() > 16384
        || url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.port().is_some()
        || url.host_str().is_none()
        || url
            .host_str()
            .and_then(|s| s.trim_matches(['[', ']']).parse::<IpAddr>().ok())
            .is_some_and(|ip| !super::public_addresses::allowed_ip(ip, false))
    {
        return Err(());
    }
    Ok(url)
}
