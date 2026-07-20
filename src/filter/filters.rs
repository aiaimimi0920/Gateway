// ---------------------------------------------------------------------------
// Content filter implementations
//
// Rust port of the TypeScript content-filter.ts.
// Three concrete filters ship by default:
//   • KeywordFilter  — exact/case-insensitive keyword matching
//   • PiiFilter      — regex-based PII detection with redaction
//   • RegexFilter    — arbitrary named regex patterns with per-pattern severity
// ---------------------------------------------------------------------------

use regex::Regex;
use std::sync::OnceLock;

// ---------------------------------------------------------------------------
// Severity
// ---------------------------------------------------------------------------

/// Severity level of a filter match. Ordered from least to most severe so that
/// `Ord` comparison works naturally (`Low < Medium < High < Critical`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Low => write!(f, "low"),
            Severity::Medium => write!(f, "medium"),
            Severity::High => write!(f, "high"),
            Severity::Critical => write!(f, "critical"),
        }
    }
}

// ---------------------------------------------------------------------------
// FilterMatch
// ---------------------------------------------------------------------------

/// Describes a single hit produced by a content filter.
#[derive(Debug, Clone)]
pub struct FilterMatch {
    /// Name of the filter that produced this match.
    pub filter_name: String,
    /// Severity of this particular hit.
    pub severity: Severity,
    /// The substring or redacted placeholder that was matched.
    pub matched: String,
    /// Optional surrounding context (e.g. surrounding text, field name).
    pub context: Option<String>,
}

// ---------------------------------------------------------------------------
// ContentFilter trait
// ---------------------------------------------------------------------------

/// A single content filter.  Implementations must be `Send + Sync` so they can
/// live in shared `Arc` state and be called concurrently.
pub trait ContentFilter: Send + Sync {
    /// Human-readable name, used in [`FilterMatch::filter_name`].
    fn name(&self) -> &str;

    /// Scan `content` and return zero or more matches.
    fn scan(&self, content: &str) -> Vec<FilterMatch>;
}

// ---------------------------------------------------------------------------
// KeywordFilter
// ---------------------------------------------------------------------------

/// Blocks or flags content that contains any of the configured keywords.
pub struct KeywordFilter {
    pub keywords: Vec<String>,
    pub severity: Severity,
    pub case_sensitive: bool,
}

impl KeywordFilter {
    pub fn new(keywords: Vec<String>, severity: Severity, case_sensitive: bool) -> Self {
        Self {
            keywords,
            severity,
            case_sensitive,
        }
    }
}

impl ContentFilter for KeywordFilter {
    fn name(&self) -> &str {
        "keyword_filter"
    }

    fn scan(&self, content: &str) -> Vec<FilterMatch> {
        let haystack: std::borrow::Cow<str> = if self.case_sensitive {
            std::borrow::Cow::Borrowed(content)
        } else {
            std::borrow::Cow::Owned(content.to_lowercase())
        };

        self.keywords
            .iter()
            .filter_map(|kw| {
                let needle = if self.case_sensitive {
                    kw.clone()
                } else {
                    kw.to_lowercase()
                };
                if haystack.contains(needle.as_str()) {
                    Some(FilterMatch {
                        filter_name: self.name().to_string(),
                        severity: self.severity,
                        matched: kw.clone(),
                        context: None,
                    })
                } else {
                    None
                }
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// PII regex patterns (compiled once)
// ---------------------------------------------------------------------------

struct PiiPattern {
    name: &'static str,
    regex: Regex,
    severity: Severity,
}

fn pii_patterns() -> &'static Vec<PiiPattern> {
    static PATTERNS: OnceLock<Vec<PiiPattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            PiiPattern {
                name: "email",
                regex: Regex::new(r"[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}").unwrap(),
                severity: Severity::High,
            },
            PiiPattern {
                name: "us_phone",
                regex: Regex::new(
                    r"(?:\+?1[-.\s]?)?(?:\(?\d{3}\)?[-.\s]?)?\d{3}[-.\s]?\d{4}",
                )
                .unwrap(),
                severity: Severity::High,
            },
            PiiPattern {
                name: "credit_card",
                // Major card issuers: Visa, MC, Amex, Discover — 13–19 digits with
                // optional spaces or dashes.
                regex: Regex::new(
                    r"(?:4\d{3}|5[1-5]\d{2}|6011|3[47]\d{2}|3(?:0[0-5]|[68]\d)\d)[\s\-]?\d{4}[\s\-]?\d{4}[\s\-]?\d{0,4}",
                )
                .unwrap(),
                severity: Severity::Critical,
            },
            PiiPattern {
                name: "us_ssn",
                regex: Regex::new(r"\b\d{3}[-\s]\d{2}[-\s]\d{4}\b").unwrap(),
                severity: Severity::Critical,
            },
            PiiPattern {
                name: "ipv4_address",
                regex: Regex::new(
                    r"\b(?:(?:25[0-5]|2[0-4]\d|[01]?\d\d?)\.){3}(?:25[0-5]|2[0-4]\d|[01]?\d\d?)\b",
                )
                .unwrap(),
                severity: Severity::Medium,
            },
        ]
    })
}

// ---------------------------------------------------------------------------
// PiiFilter
// ---------------------------------------------------------------------------

/// Detects PII (email, phone, credit card, SSN, IP) and **redacts** the matched
/// text so downstream logs never contain the raw value.
pub struct PiiFilter;

impl PiiFilter {
    /// Redact a matched string, keeping the first visible character and domain
    /// for emails, masking the rest with `*`.
    ///
    /// Examples:
    ///   `john@example.com`  → `j***@example.com`
    ///   `555-867-5309`      → `***`
    fn redact(name: &str, raw: &str) -> String {
        match name {
            "email" => {
                // Keep first char + local part redacted + @domain
                if let Some(at) = raw.find('@') {
                    let local = &raw[..at];
                    let domain = &raw[at..];
                    if local.len() <= 1 {
                        format!("{}***{}", local, domain)
                    } else {
                        format!("{}***{}", &local[..1], domain)
                    }
                } else {
                    "***".to_string()
                }
            }
            _ => "***".to_string(),
        }
    }
}

impl ContentFilter for PiiFilter {
    fn name(&self) -> &str {
        "pii_filter"
    }

    fn scan(&self, content: &str) -> Vec<FilterMatch> {
        let mut matches = Vec::new();
        for pattern in pii_patterns() {
            for cap in pattern.regex.find_iter(content) {
                let raw = cap.as_str();
                let redacted = Self::redact(pattern.name, raw);
                matches.push(FilterMatch {
                    filter_name: self.name().to_string(),
                    severity: pattern.severity,
                    matched: redacted,
                    context: Some(pattern.name.to_string()),
                });
            }
        }
        matches
    }
}

// ---------------------------------------------------------------------------
// RegexFilter
// ---------------------------------------------------------------------------

/// A configurable filter backed by a list of `(compiled_regex, severity)` pairs.
pub struct RegexFilter {
    pub name: String,
    pub patterns: Vec<(Regex, Severity)>,
}

impl RegexFilter {
    pub fn new(name: impl Into<String>, patterns: Vec<(Regex, Severity)>) -> Self {
        Self {
            name: name.into(),
            patterns,
        }
    }
}

impl ContentFilter for RegexFilter {
    fn name(&self) -> &str {
        &self.name
    }

    fn scan(&self, content: &str) -> Vec<FilterMatch> {
        let mut matches = Vec::new();
        for (regex, severity) in &self.patterns {
            for cap in regex.find_iter(content) {
                matches.push(FilterMatch {
                    filter_name: self.name.clone(),
                    severity: *severity,
                    matched: cap.as_str().to_string(),
                    context: None,
                });
            }
        }
        matches
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── Severity ordering ────────────────────────────────────────────────

    #[test]
    fn severity_ordering_is_correct() {
        assert!(Severity::Low < Severity::Medium);
        assert!(Severity::Medium < Severity::High);
        assert!(Severity::High < Severity::Critical);
    }

    // ── KeywordFilter ────────────────────────────────────────────────────

    #[test]
    fn keyword_filter_case_insensitive_matches() {
        let f = KeywordFilter::new(vec!["bomb".to_string()], Severity::High, false);
        let hits = f.scan("I want to BOMB the server");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].severity, Severity::High);
        assert_eq!(hits[0].matched, "bomb");
    }

    #[test]
    fn keyword_filter_case_sensitive_no_match() {
        let f = KeywordFilter::new(vec!["Bomb".to_string()], Severity::High, true);
        let hits = f.scan("I want to bomb the server");
        assert!(hits.is_empty());
    }

    #[test]
    fn keyword_filter_case_sensitive_match() {
        let f = KeywordFilter::new(vec!["Bomb".to_string()], Severity::High, true);
        let hits = f.scan("I want to Bomb the server");
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn keyword_filter_no_match_returns_empty() {
        let f = KeywordFilter::new(vec!["malware".to_string()], Severity::Medium, false);
        let hits = f.scan("This is a perfectly benign sentence.");
        assert!(hits.is_empty());
    }

    #[test]
    fn keyword_filter_multiple_keywords() {
        let f = KeywordFilter::new(
            vec!["hack".to_string(), "exploit".to_string()],
            Severity::High,
            false,
        );
        let hits = f.scan("I'll hack the exploit");
        assert_eq!(hits.len(), 2);
    }

    // ── PiiFilter ────────────────────────────────────────────────────────

    #[test]
    fn pii_filter_detects_email() {
        let f = PiiFilter;
        let hits = f.scan("Contact me at john.doe@example.com please.");
        let email_hits: Vec<_> = hits
            .iter()
            .filter(|h| h.context.as_deref() == Some("email"))
            .collect();
        assert!(!email_hits.is_empty());
        // Redacted — should not contain the raw local part
        assert!(!email_hits[0].matched.contains("john.doe"));
        assert!(email_hits[0].matched.contains("@example.com"));
    }

    #[test]
    fn pii_filter_redacts_email_correctly() {
        assert_eq!(
            PiiFilter::redact("email", "john@example.com"),
            "j***@example.com"
        );
        assert_eq!(PiiFilter::redact("email", "a@b.com"), "a***@b.com");
    }

    #[test]
    fn pii_filter_detects_ssn() {
        let f = PiiFilter;
        let hits = f.scan("My SSN is 123-45-6789.");
        let ssn_hits: Vec<_> = hits
            .iter()
            .filter(|h| h.context.as_deref() == Some("us_ssn"))
            .collect();
        assert!(!ssn_hits.is_empty());
        assert_eq!(ssn_hits[0].severity, Severity::Critical);
    }

    #[test]
    fn pii_filter_detects_ipv4() {
        let f = PiiFilter;
        let hits = f.scan("Server IP: 192.168.1.1");
        let ip_hits: Vec<_> = hits
            .iter()
            .filter(|h| h.context.as_deref() == Some("ipv4_address"))
            .collect();
        assert!(!ip_hits.is_empty());
        assert_eq!(ip_hits[0].severity, Severity::Medium);
    }

    #[test]
    fn pii_filter_clean_text_returns_empty() {
        let f = PiiFilter;
        let hits = f.scan("The quick brown fox jumps over the lazy dog.");
        // No PII expected in this text
        assert!(hits.is_empty());
    }

    // ── RegexFilter ──────────────────────────────────────────────────────

    #[test]
    fn regex_filter_matches_pattern() {
        let f = RegexFilter::new(
            "profanity",
            vec![(Regex::new(r"(?i)\bbadword\b").unwrap(), Severity::Medium)],
        );
        let hits = f.scan("He said BADWORD loudly.");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].matched.to_lowercase(), "badword");
    }

    #[test]
    fn regex_filter_no_match_returns_empty() {
        let f = RegexFilter::new(
            "profanity",
            vec![(Regex::new(r"(?i)\bbadword\b").unwrap(), Severity::Medium)],
        );
        let hits = f.scan("Everything is fine here.");
        assert!(hits.is_empty());
    }

    #[test]
    fn regex_filter_returns_filter_name() {
        let f = RegexFilter::new("custom_filter", vec![]);
        assert_eq!(f.name(), "custom_filter");
    }
}
