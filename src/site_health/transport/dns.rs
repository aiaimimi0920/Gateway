//! Recover only synthetic Fake-IP DNS answers; never connect to a reserved address.
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

pub(super) fn fake_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => ip.octets()[0] == 198 && (18..=19).contains(&ip.octets()[1]),
        IpAddr::V6(ip) => ip.to_ipv4_mapped().is_some_and(|ip| fake_ip(ip.into())),
    }
}

fn recoverable(host: &str, addresses: &[SocketAddr]) -> bool {
    let host = host.trim_end_matches('.');
    let local = [
        "local",
        "localhost",
        "internal",
        "lan",
        "home",
        "arpa",
        "invalid",
        "test",
    ];
    host.len() <= 253
        && host.contains('.')
        && host.parse::<IpAddr>().is_err()
        && !local.iter().any(|suffix| {
            host.eq_ignore_ascii_case(suffix)
                || host.to_ascii_lowercase().ends_with(&format!(".{suffix}"))
        })
        && !addresses.is_empty()
        && addresses.len() <= 16
        && addresses.iter().all(|address| fake_ip(address.ip()))
}

pub(super) async fn resolve(host: &str, port: u16) -> anyhow::Result<Vec<SocketAddr>> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(vec![SocketAddr::new(ip, port)]);
    }
    let addresses: Vec<_> = tokio::time::timeout(
        Duration::from_secs(3),
        tokio::net::lookup_host((host, port)),
    )
    .await??
    .take(17)
    .collect();
    if !recoverable(host, &addresses) {
        return Ok(addresses);
    }
    // Fixed resolver and IP bootstrap, authenticated by normal TLS for dns.google.
    // Only this fixed resolver may use the trusted proxy contract; arbitrary sites stay direct.
    // No monitoring/account secrets or redirects. Direct/bypassed bootstrap does not use system DNS.
    let client = crate::http_client::builder()
        .redirect(rquest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(4))
        .resolve_to_addrs("dns.google", vec!["8.8.8.8:443".parse::<SocketAddr>()?])
        .build()?;
    let query = |kind| query(&client, host, port, kind);
    let (mut v4, v6) = tokio::try_join!(query(1), query(28))?;
    v4.extend(v6);
    v4.sort_unstable();
    v4.dedup();
    anyhow::ensure!(
        !v4.is_empty() && v4.len() <= 16,
        "Invalid public DNS address count"
    );
    Ok(v4)
}

async fn query(
    client: &rquest::Client,
    host: &str,
    port: u16,
    kind: u64,
) -> anyhow::Result<Vec<SocketAddr>> {
    let mut url = url::Url::parse("https://dns.google/resolve")?;
    url.query_pairs_mut()
        .append_pair("name", host)
        .append_pair("type", &kind.to_string());
    for attempt in 0..2 {
        match super::bounded_response(
            client
                .get(url.as_str())
                .header("Accept", "application/dns-json"),
            16_384,
        )
        .await
        {
            Ok((_, body)) => return parse(&serde_json::from_slice(&body)?, host, port, kind),
            // Repeat only the same idempotent GET on transient I/O. Never retry unsafe answers,
            // HTTP refusals, redirects or oversized bodies, or spawn work beyond the plan deadline.
            Err((status, _))
                if attempt == 0
                    && matches!(
                        status.as_str(),
                        "timeout" | "transport_error" | "body_interrupted"
                    ) =>
            {
                ()
            }
            Err(_) => anyhow::bail!("Public DNS recovery request failed"),
        }
    }
    anyhow::bail!("Public DNS recovery request failed")
}

fn parse(
    body: &serde_json::Value,
    host: &str,
    port: u16,
    kind: u64,
) -> anyhow::Result<Vec<SocketAddr>> {
    anyhow::ensure!(
        body["Status"] == 0 && body["TC"] == false,
        "Invalid public DNS status"
    );
    let question = body["Question"]
        .as_array()
        .filter(|questions| questions.len() == 1)
        .ok_or_else(|| anyhow::anyhow!("Invalid public DNS question"))?;
    anyhow::ensure!(
        question[0]["type"] == kind
            && question[0]["name"].as_str().is_some_and(|name| name
                .trim_end_matches('.')
                .eq_ignore_ascii_case(host.trim_end_matches('.'))),
        "Mismatched public DNS question"
    );
    let empty = Vec::new();
    let answers = match body.get("Answer") {
        None => &empty,
        Some(value) => value
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("Invalid public DNS answers"))?,
    };
    anyhow::ensure!(answers.len() <= 32, "Too many public DNS records");
    let mut addresses = Vec::new();
    for answer in answers {
        match answer["type"].as_u64() {
            Some(record_type @ (1 | 28)) => {
                let ip: IpAddr = answer["data"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Invalid public DNS address"))?
                    .parse()?;
                anyhow::ensure!(
                    super::allowed_ip(ip, false),
                    "Public DNS returned a prohibited address"
                );
                anyhow::ensure!(
                    record_type == kind
                        && ((record_type == 1 && ip.is_ipv4())
                            || (record_type == 28 && ip.is_ipv6())),
                    "Mismatched public DNS address type"
                );
                addresses.push(SocketAddr::new(ip, port));
            }
            Some(5) => (), // Resolver follows CNAMEs; never resolve them again through the system.
            _ => anyhow::bail!("Unexpected public DNS record type"),
        }
    }
    anyhow::ensure!(addresses.len() <= 16, "Too many public DNS addresses");
    Ok(addresses)
}

#[cfg(test)]
#[path = "dns_tests.rs"]
mod tests;
