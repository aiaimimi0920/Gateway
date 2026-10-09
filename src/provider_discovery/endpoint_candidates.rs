//! Conventional, same-origin aliases only; preserve explicit deployment prefixes.
use super::DiscoveredProtocol as P;

pub const MAX_BASES: usize = 8;

pub fn bases(source: &str, protocol: P) -> anyhow::Result<Vec<String>> {
    let url = super::transport::checked_url(source)?;
    let source = url.as_str().trim_end_matches('/');
    let root = [
        "/compatible-mode/v1",
        "/api/v1",
        "/v1beta",
        "/v1",
        "/v2",
        "/api",
    ]
    .iter()
    .find_map(|tail| source.strip_suffix(tail))
    .unwrap_or(source);
    let (versions, aliases): (&[&str], &[&str]) = match protocol {
        P::GeminiGenerateContent | P::GeminiInteractions => (&["v1beta", "v1"], &["gemini"]),
        P::Messages => (&["v1"], &["claude", "anthropic"]),
        P::ChatCompletions | P::Responses | P::Completions => (&["v1"], &["openai"]),
        P::OllamaChat | P::OllamaGenerate => (&["api"], &[]),
        P::CohereChat => (&["v2"], &[]),
        P::BedrockConverse => (&[], &[]),
        P::DashscopeText | P::DashscopeMultimodal => (&["api/v1"], &["dashscope"]),
    };
    let mut result = Vec::new();
    let mut push = |base: String| {
        if !result.contains(&base) {
            result.push(base);
        }
    };
    // Keep the existing preferred version first, then preserve the supplied URL.
    if let Some(version) = versions.first() {
        push(format!("{root}/{version}"));
    }
    push(source.to_owned());
    for version in versions {
        push(format!("{root}/{version}"));
    }
    push(root.to_owned());
    // An explicit /proxy/gemini[/v1beta] stays in that scope; never append
    // /gemini/gemini or probe sibling deployments without an explicit root.
    let has_alias = ["openai", "claude", "anthropic", "gemini", "dashscope"]
        .iter()
        .any(|alias| root.ends_with(&format!("/{alias}")));
    if !has_alias {
        for alias in aliases {
            for version in versions {
                push(format!("{root}/{alias}/{version}"));
            }
            push(format!("{root}/{alias}"));
        }
    }
    anyhow::ensure!(
        result.len() <= MAX_BASES,
        "Discovery endpoint candidate limit exceeded."
    );
    Ok(result)
}
