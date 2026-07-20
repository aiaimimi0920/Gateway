use crate::protocol::gemini_canvas;
use crate::upstream::response_preview_helpers::truncate_response_preview;
use std::time::{SystemTime, UNIX_EPOCH};

fn unix_tuple_from_system_time(value: SystemTime) -> (i64, i64) {
    value
        .duration_since(UNIX_EPOCH)
        .map(|duration| {
            (
                duration.as_secs() as i64,
                i64::from(duration.subsec_nanos()),
            )
        })
        .unwrap_or_default()
}

fn normalize_gemini_canvas_title_match_text(value: &str) -> String {
    value
        .trim()
        .replace('\n', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

pub(crate) fn preview_gemini_canvas_conversation_entries(
    entries: &[gemini_canvas::GeminiCanvasConversationListEntry],
    limit: usize,
) -> String {
    if entries.is_empty() {
        return "<none>".to_string();
    }
    entries
        .iter()
        .take(limit)
        .map(|entry| {
            format!(
                "{}@{}.{}:{}",
                entry.conversation_id,
                entry.updated_at_secs,
                entry.updated_at_nanos,
                truncate_response_preview(entry.title.as_str(), 60)
            )
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

pub(crate) fn select_gemini_canvas_recent_conversation_entry<'a>(
    entries: &'a [gemini_canvas::GeminiCanvasConversationListEntry],
    prompt: &str,
    request_started_at: SystemTime,
) -> Option<&'a gemini_canvas::GeminiCanvasConversationListEntry> {
    if entries.is_empty() {
        return None;
    }
    let (request_started_secs, request_started_nanos) =
        unix_tuple_from_system_time(request_started_at);
    let lower_bound_secs = request_started_secs.saturating_sub(300);
    let lower_bound = (lower_bound_secs, 0i64);
    let eligible = entries
        .iter()
        .filter(|entry| (entry.updated_at_secs, entry.updated_at_nanos) >= lower_bound)
        .collect::<Vec<_>>();
    if eligible.is_empty() {
        return None;
    }

    let normalized_prompt = normalize_gemini_canvas_title_match_text(prompt);
    let prompt_hint = normalized_prompt
        .split_whitespace()
        .find(|segment| segment.chars().count() >= 6)
        .map(str::to_string)
        .or_else(|| {
            let compact = normalized_prompt.replace(' ', "");
            (compact.chars().count() >= 6).then(|| compact)
        });

    if let Some(hint) = prompt_hint.as_deref() {
        if let Some(entry) = eligible.iter().find(|entry| {
            normalize_gemini_canvas_title_match_text(entry.title.as_str()).contains(hint)
        }) {
            return Some(*entry);
        }
    }

    eligible
        .iter()
        .copied()
        .find(|entry| {
            (entry.updated_at_secs, entry.updated_at_nanos)
                >= (request_started_secs, request_started_nanos)
        })
        .or_else(|| eligible.first().copied())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn make_conversation_entry(
        conversation_id: &str,
        title: &str,
        updated_at_secs: i64,
        updated_at_nanos: i64,
    ) -> gemini_canvas::GeminiCanvasConversationListEntry {
        gemini_canvas::GeminiCanvasConversationListEntry {
            conversation_id: conversation_id.to_string(),
            title: title.to_string(),
            response_id: Some("r_test".to_string()),
            updated_at_secs,
            updated_at_nanos,
        }
    }

    #[test]
    fn select_gemini_canvas_recent_conversation_entry_prefers_prompt_match() {
        let request_started_at = UNIX_EPOCH + Duration::from_secs(1_000);
        let entries = vec![
            make_conversation_entry("c_first", "older unrelated idea", 995, 0),
            make_conversation_entry("c_second", "Bright cube storyboard", 996, 0),
            make_conversation_entry("c_third", "latest but unrelated", 1_001, 0),
        ];

        let selected = select_gemini_canvas_recent_conversation_entry(
            &entries,
            "bright cube close-up render",
            request_started_at,
        )
        .expect("matched entry");

        assert_eq!(selected.conversation_id, "c_second");
    }

    #[test]
    fn preview_gemini_canvas_conversation_entries_truncates_titles() {
        let entries = vec![make_conversation_entry(
            "c_preview",
            "This title is intentionally long enough to exceed the preview truncation boundary",
            1_000,
            0,
        )];

        let preview = preview_gemini_canvas_conversation_entries(&entries, 5);

        assert!(preview.starts_with("c_preview@1000.0:"));
        assert!(preview.contains("This title is intentionally long enough to exceed the prev"));
        assert!(!preview.contains("truncation boundary"));
    }

    #[test]
    fn select_gemini_canvas_recent_conversation_entry_filters_old_and_empty_ids() {
        let request_started_at = UNIX_EPOCH + Duration::from_secs(1_000);
        let entries = vec![
            make_conversation_entry("", "too old and unusable", 600, 0),
            make_conversation_entry("c_recent", "recent usable", 1_002, 0),
            make_conversation_entry("c_old", "old usable", 650, 0),
        ];

        let selected = select_gemini_canvas_recent_conversation_entry(
            &entries,
            "totally different prompt",
            request_started_at,
        )
        .expect("recent entry");

        assert_eq!(selected.conversation_id, "c_recent");
        assert_eq!(selected.app_path().as_deref(), Some("/app/recent"));
    }
}
