//! Diverse, bounded generation samples; names prioritize probes, never prove support.
use super::DiscoveredProtocol as P;
use std::collections::HashSet;

pub(super) fn models(models: &[String], protocol: P) -> Vec<&String> {
    let mut ordered = models.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|model| {
        let name = model
            .rsplit('/')
            .next()
            .unwrap_or(model)
            .to_ascii_lowercase();
        let non_text = ["embed", "rerank", "image", "tts", "whisper"]
            .iter()
            .any(|marker| name.contains(marker));
        let preferred = match protocol {
            P::GeminiGenerateContent | P::GeminiInteractions => name.starts_with("gemini"),
            P::Messages => name.starts_with("claude"),
            P::CohereChat => name.starts_with("command"),
            P::DashscopeText => name.starts_with("qwen"),
            P::DashscopeMultimodal => name.starts_with("qwen") && name.contains("vl"),
            _ => true,
        };
        (non_text, !preferred)
    });
    let mut families = HashSet::new();
    let mut selected = Vec::new();
    // Three adjacent variants of a broken upstream channel must not consume
    // every sample. Prefer one per family, then fill from the remaining models.
    for model in &ordered {
        let name = model
            .rsplit('/')
            .next()
            .unwrap_or(model)
            .to_ascii_lowercase();
        let family = name
            .split(['-', '_', '.'])
            .next()
            .unwrap_or(&name)
            .to_owned();
        if families.insert(family) {
            selected.push(*model);
            if selected.len() == 3 {
                return selected;
            }
        }
    }
    for model in ordered {
        if !selected.contains(&model) {
            selected.push(model);
            if selected.len() == 3 {
                break;
            }
        }
    }
    selected
}

#[cfg(test)]
mod tests {
    use super::*;

    fn select(names: &[&str], protocol: P) -> Vec<String> {
        let names = names
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<Vec<_>>();
        models(&names, protocol).into_iter().cloned().collect()
    }

    #[test]
    fn bounds_and_diversifies_without_inventing_models() {
        assert_eq!(
            select(
                &["deepseek-a", "deepseek-b", "deepseek-c", "custom-chat"],
                P::ChatCompletions
            ),
            ["deepseek-a", "custom-chat", "deepseek-b"]
        );
        assert_eq!(select(&["a-1", "a-2"], P::ChatCompletions), ["a-1", "a-2"]);
        assert!(select(&[], P::ChatCompletions).is_empty());
    }

    #[test]
    fn protocol_preference_and_text_samples_still_win() {
        assert_eq!(
            select(
                &[
                    "text-embedding",
                    "gpt-image",
                    "qwen-chat",
                    "vendor/claude-haiku"
                ],
                P::Messages
            )[0],
            "vendor/claude-haiku"
        );
        assert_eq!(
            select(
                &["gpt-image", "qwen-chat", "gemini-flash"],
                P::GeminiGenerateContent
            )[0],
            "gemini-flash"
        );
    }
}
