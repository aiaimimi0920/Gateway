//! Evidence-relative answer quality. Wrong answers never become connectivity failures.
use super::ProviderPayloadProbeStatus;
use crate::credential_test_policy::CredentialTestCase;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestCaseResult {
    pub case_id: String,
    pub name: String,
    pub model: String,
    pub difficulty: u8,
    pub status: String,
    pub answer: Option<String>,
    pub expected_answer: Option<String>,
    pub correct: Option<bool>,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelAssessment {
    pub model: String,
    pub callable: bool,
    pub completed_count: usize,
    pub graded_count: usize,
    pub correct_count: usize,
    pub score: Option<u8>,
    pub capability_level: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measurement: Option<super::test_measurement::TestModelMeasurement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestAssessment {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_id: Option<String>,
    pub policy_source: String,
    pub mode: String,
    pub models: Vec<ModelAssessment>,
    pub cases: Vec<TestCaseResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestRunResult {
    pub credential_id: String,
    pub provider_id: String,
    pub probe_point: String,
    pub status: ProviderPayloadProbeStatus,
    pub message: String,
    pub checked_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assessment: Option<TestAssessment>,
}

pub(crate) fn checked_at() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .expect("timestamp")
}

pub(super) fn case_result(
    case: &CredentialTestCase,
    model: &str,
    report: &super::ProviderPayloadProbeReport,
) -> TestCaseResult {
    let passed = report.status == ProviderPayloadProbeStatus::Passed;
    let correct = case
        .expected_answer
        .as_ref()
        .filter(|_| passed)
        .map(|expected| {
            report.answer.as_deref().is_some_and(|answer| {
                answer.trim().to_lowercase() == expected.trim().to_lowercase()
            })
        });
    TestCaseResult {
        case_id: case.id.clone(),
        name: case.name.clone(),
        model: model.into(),
        difficulty: case.difficulty,
        status: match report.status {
            ProviderPayloadProbeStatus::Passed => "passed",
            ProviderPayloadProbeStatus::Failed => "failed",
            ProviderPayloadProbeStatus::Unsupported => "unsupported",
        }
        .into(),
        answer: report
            .answer
            .as_ref()
            .map(|answer| answer.chars().take(320).collect()),
        expected_answer: case
            .expected_answer
            .as_ref()
            .map(|answer| answer.chars().take(320).collect()),
        correct,
        message: report.message.clone(),
        elapsed_ms: None,
    }
}

pub(super) fn model_assessment(model: &str, cases: &[TestCaseResult]) -> ModelAssessment {
    let rows: Vec<_> = cases.iter().filter(|case| case.model == model).collect();
    let completed_count = rows.iter().filter(|case| case.status == "passed").count();
    let graded_count = rows.iter().filter(|case| case.correct.is_some()).count();
    let correct_count = rows
        .iter()
        .filter(|case| case.correct == Some(true))
        .count();
    let score = (graded_count > 0).then(|| (correct_count * 100 / graded_count) as u8);
    let highest = rows
        .iter()
        .filter(|case| case.correct == Some(true))
        .map(|case| case.difficulty)
        .max()
        .unwrap_or(0);
    let level = match score {
        None => "unrated",
        Some(_) if rows.iter().any(|case| case.status != "passed") => "incomplete",
        Some(score) if score < 80 => "below-standard",
        Some(_) => match highest {
            3 => "advanced",
            2 => "intermediate",
            1 => "basic",
            _ => "below-standard",
        },
    };
    ModelAssessment {
        model: model.into(),
        callable: completed_count > 0,
        completed_count,
        graded_count,
        correct_count,
        score,
        capability_level: level.into(),
        measurement: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wrong_answer_is_callable_but_below_standard_and_ungraded_is_not_ranked() {
        let case = CredentialTestCase {
            id: "math".into(),
            name: "math".into(),
            prompt: "1+1".into(),
            expected_answer: Some("2".into()),
            difficulty: 2,
            enabled: true,
        };
        let report = super::super::ProviderPayloadProbeReport {
            status: ProviderPayloadProbeStatus::Passed,
            message: "completed".into(),
            answer: Some("3".into()),
        };
        let result = case_result(&case, "model", &report);
        let assessment = model_assessment("model", &[result]);
        assert!(assessment.callable);
        assert_eq!(assessment.score, Some(0));
        assert_eq!(assessment.capability_level, "below-standard");
        let ungraded = CredentialTestCase {
            expected_answer: None,
            ..case
        };
        assert_eq!(
            model_assessment("model", &[case_result(&ungraded, "model", &report)]).capability_level,
            "unrated"
        );
    }
}
