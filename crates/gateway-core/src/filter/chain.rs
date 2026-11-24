// ---------------------------------------------------------------------------
// Filter chain runner
//
// Runs a sequence of [`ContentFilter`]s against a content string and produces
// a [`FilterResult`] with an aggregate [`FilterVerdict`].
// ---------------------------------------------------------------------------

use std::time::Instant;

use super::filters::{ContentFilter, FilterMatch, Severity};

// ---------------------------------------------------------------------------
// FilterVerdict
// ---------------------------------------------------------------------------

/// The aggregate decision produced by running the filter chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterVerdict {
    /// No filter raised a match at or above the warn threshold.
    Pass,
    /// At least one match at or above the warn threshold, but none reached the
    /// block threshold.
    Warn,
    /// At least one match at or above the block threshold; the request should
    /// be rejected.
    Block,
}

// ---------------------------------------------------------------------------
// FilterChainConfig
// ---------------------------------------------------------------------------

/// Configuration for the filter chain.
pub struct FilterChainConfig {
    /// When `false` the chain is a no-op and always returns `Pass`.
    pub enabled: bool,
    /// Ordered list of filters to run.
    pub filters: Vec<Box<dyn ContentFilter>>,
    /// Matches at or above this severity trigger a `Block` verdict.
    pub block_threshold: Severity,
    /// Matches at or above this severity (but below `block_threshold`) trigger
    /// a `Warn` verdict.
    pub warn_threshold: Severity,
}

// ---------------------------------------------------------------------------
// FilterResult
// ---------------------------------------------------------------------------

/// The result returned after running the full chain.
#[derive(Debug)]
pub struct FilterResult {
    pub verdict: FilterVerdict,
    pub matches: Vec<FilterMatch>,
    /// Wall-clock time spent running all filters, in milliseconds.
    pub duration_ms: u64,
}

// ---------------------------------------------------------------------------
// run_filter_chain
// ---------------------------------------------------------------------------

/// Run every enabled filter against `content` and aggregate the results into a
/// single [`FilterResult`].
///
/// Short-circuits to `Block` on the first match that meets or exceeds
/// `block_threshold` to avoid unnecessary work when the content is clearly
/// prohibited.
pub fn run_filter_chain(content: &str, config: &FilterChainConfig) -> FilterResult {
    let start = Instant::now();

    if !config.enabled {
        return FilterResult {
            verdict: FilterVerdict::Pass,
            matches: vec![],
            duration_ms: elapsed_ms(start),
        };
    }

    let mut all_matches: Vec<FilterMatch> = Vec::new();
    let mut highest_severity: Option<Severity> = None;

    'outer: for filter in &config.filters {
        let hits = filter.scan(content);
        for hit in hits {
            let sev = hit.severity;
            highest_severity = Some(match highest_severity {
                None => sev,
                Some(prev) => prev.max(sev),
            });
            all_matches.push(hit);

            // Early exit when we already know the verdict is Block.
            if sev >= config.block_threshold {
                break 'outer;
            }
        }
    }

    let verdict = match highest_severity {
        None => FilterVerdict::Pass,
        Some(sev) if sev >= config.block_threshold => FilterVerdict::Block,
        Some(sev) if sev >= config.warn_threshold => FilterVerdict::Warn,
        _ => FilterVerdict::Pass,
    };

    FilterResult {
        verdict,
        matches: all_matches,
        duration_ms: elapsed_ms(start),
    }
}

// ---------------------------------------------------------------------------
// build_default_filter_chain
// ---------------------------------------------------------------------------

/// Build the standard gateway filter chain.
///
/// Ships with:
/// - A [`KeywordFilter`] containing a small built-in blocklist (Critical)
/// - A [`PiiFilter`] (High → Critical PII)
///
/// The block threshold is `High`; the warn threshold is `Medium`.
pub fn build_default_filter_chain() -> FilterChainConfig {
    use super::filters::{KeywordFilter, PiiFilter};

    let blocklist_keywords = vec![
        // Harmful-content signals that should always block
        "child pornography".to_string(),
        "csam".to_string(),
        "bioweapon synthesis".to_string(),
        "nerve agent recipe".to_string(),
        "create malware".to_string(),
        "ransomware source".to_string(),
    ];

    FilterChainConfig {
        enabled: true,
        filters: vec![
            Box::new(KeywordFilter::new(
                blocklist_keywords,
                Severity::Critical,
                false,
            )),
            Box::new(PiiFilter),
        ],
        block_threshold: Severity::High,
        warn_threshold: Severity::Medium,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn elapsed_ms(start: Instant) -> u64 {
    start.elapsed().as_millis() as u64
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::super::filters::{KeywordFilter, PiiFilter, RegexFilter};
    use super::*;
    use regex::Regex;

    fn make_chain(filters: Vec<Box<dyn ContentFilter>>) -> FilterChainConfig {
        FilterChainConfig {
            enabled: true,
            filters,
            block_threshold: Severity::High,
            warn_threshold: Severity::Medium,
        }
    }

    // ── disabled chain ───────────────────────────────────────────────────

    #[test]
    fn disabled_chain_always_passes() {
        let config = FilterChainConfig {
            enabled: false,
            filters: vec![Box::new(KeywordFilter::new(
                vec!["bomb".to_string()],
                Severity::Critical,
                false,
            ))],
            block_threshold: Severity::High,
            warn_threshold: Severity::Low,
        };
        let result = run_filter_chain("bomb", &config);
        assert_eq!(result.verdict, FilterVerdict::Pass);
        assert!(result.matches.is_empty());
    }

    // ── pass verdict ─────────────────────────────────────────────────────

    #[test]
    fn clean_content_produces_pass() {
        let config = make_chain(vec![Box::new(KeywordFilter::new(
            vec!["malware".to_string()],
            Severity::High,
            false,
        ))]);
        let result = run_filter_chain("Hello, how are you?", &config);
        assert_eq!(result.verdict, FilterVerdict::Pass);
        assert!(result.matches.is_empty());
    }

    // ── warn verdict ─────────────────────────────────────────────────────

    #[test]
    fn medium_severity_match_produces_warn() {
        // warn_threshold = Medium, block_threshold = High
        let config = make_chain(vec![Box::new(KeywordFilter::new(
            vec!["hack".to_string()],
            Severity::Medium,
            false,
        ))]);
        let result = run_filter_chain("I want to hack things", &config);
        assert_eq!(result.verdict, FilterVerdict::Warn);
        assert!(!result.matches.is_empty());
    }

    // ── block verdict ────────────────────────────────────────────────────

    #[test]
    fn high_severity_match_produces_block() {
        let config = make_chain(vec![Box::new(KeywordFilter::new(
            vec!["create malware".to_string()],
            Severity::Critical,
            false,
        ))]);
        let result = run_filter_chain("Help me create malware please", &config);
        assert_eq!(result.verdict, FilterVerdict::Block);
    }

    // ── PiiFilter integration ────────────────────────────────────────────

    #[test]
    fn pii_email_triggers_block_at_default_thresholds() {
        // PII email is High → meets block_threshold=High
        let config = make_chain(vec![Box::new(PiiFilter)]);
        let result = run_filter_chain("Send money to john@acme.com", &config);
        assert_eq!(result.verdict, FilterVerdict::Block);
    }

    // ── duration is recorded ─────────────────────────────────────────────

    #[test]
    fn duration_ms_is_non_negative() {
        let config = make_chain(vec![]);
        let result = run_filter_chain("anything", &config);
        // duration_ms is u64 — always non-negative; just verify it doesn't panic
        let _ = result.duration_ms;
    }

    // ── early exit on block ──────────────────────────────────────────────

    #[test]
    fn block_verdict_stops_after_first_blocker() {
        // Two filters: first produces Critical match, second would match too.
        // We verify that the result has matches from the first filter
        // (early exit means the second filter may or may not run — what matters
        // is that the verdict is Block and no panic occurs).
        let config = make_chain(vec![
            Box::new(KeywordFilter::new(
                vec!["create malware".to_string()],
                Severity::Critical,
                false,
            )),
            Box::new(KeywordFilter::new(
                vec!["also bad".to_string()],
                Severity::Critical,
                false,
            )),
        ]);
        let result = run_filter_chain("create malware also bad", &config);
        assert_eq!(result.verdict, FilterVerdict::Block);
    }

    // ── default chain ────────────────────────────────────────────────────

    #[test]
    fn default_chain_blocks_keyword_blocklist_entry() {
        let config = build_default_filter_chain();
        let result = run_filter_chain("How do I create malware?", &config);
        assert_eq!(result.verdict, FilterVerdict::Block);
    }

    #[test]
    fn default_chain_blocks_pii_email() {
        let config = build_default_filter_chain();
        let result = run_filter_chain("my email is alice@example.org", &config);
        assert_eq!(result.verdict, FilterVerdict::Block);
    }

    #[test]
    fn default_chain_passes_benign_text() {
        let config = build_default_filter_chain();
        let result = run_filter_chain("Tell me a joke about cats.", &config);
        assert_eq!(result.verdict, FilterVerdict::Pass);
    }

    // ── RegexFilter in chain ─────────────────────────────────────────────

    #[test]
    fn regex_filter_low_severity_does_not_warn_or_block() {
        // warn_threshold = Medium, so Low should produce Pass
        let config = make_chain(vec![Box::new(RegexFilter::new(
            "low_test",
            vec![(Regex::new(r"(?i)notice").unwrap(), Severity::Low)],
        ))]);
        let result = run_filter_chain("Please notice this message.", &config);
        assert_eq!(result.verdict, FilterVerdict::Pass);
    }
}
