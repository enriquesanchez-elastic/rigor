//! Score calculation for test quality

use crate::{Grade, Issue, Score, ScoreBreakdown, ScoringWeights, Severity, TestCase, TestType};

use super::rules::{
    AssertionQualityRule, BoundaryConditionsRule, ErrorCoverageRule, InputVarietyRule,
    TestIsolationRule,
};

/// Penalty points per issue by severity (applied after category score).
/// Ensures files with many reported problems cannot get A/B.
const PENALTY_PER_ERROR: i32 = 5;
const PENALTY_PER_WARNING: i32 = 2;
const PENALTY_PER_INFO: i32 = 1;
const MAX_PENALTY_FROM_ERRORS: i32 = 35;
const MAX_PENALTY_FROM_WARNINGS: i32 = 40;
const MAX_PENALTY_FROM_INFO: i32 = 15;

/// Calculator for test quality scores
pub struct ScoreCalculator;

impl ScoreCalculator {
    /// Calculate the overall score from individual category scores
    pub fn calculate(breakdown: &ScoreBreakdown) -> Score {
        let total = breakdown.total();
        Score::new(total)
    }

    /// Calculate the overall score with test-type-specific weights
    pub fn calculate_weighted(breakdown: &ScoreBreakdown, test_type: TestType) -> Score {
        let weights = ScoringWeights::for_test_type(test_type);
        let total = weights.calculate_total(breakdown);
        Score::new(total)
    }

    /// Apply issue-based penalty so that files with many problems get lower grades.
    /// Errors and warnings (trivial assertions, debug code, flaky patterns, etc.)
    /// now directly reduce the final score.
    pub fn apply_issue_penalty(score: Score, issues: &[Issue]) -> Score {
        let errors = issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .count() as i32;
        let warnings = issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
            .count() as i32;
        let infos = issues
            .iter()
            .filter(|i| i.severity == Severity::Info)
            .count() as i32;

        let penalty = (errors * PENALTY_PER_ERROR).min(MAX_PENALTY_FROM_ERRORS)
            + (warnings * PENALTY_PER_WARNING).min(MAX_PENALTY_FROM_WARNINGS)
            + (infos * PENALTY_PER_INFO).min(MAX_PENALTY_FROM_INFO);

        let value = (score.value as i32 - penalty).clamp(0, 100) as u8;
        Score::new(value)
    }

    /// Calculate breakdown from tests and issues
    pub fn calculate_breakdown(
        tests: &[TestCase],
        issues: &[Issue],
        assertion_rule: &AssertionQualityRule,
        error_rule: &ErrorCoverageRule,
        boundary_rule: &BoundaryConditionsRule,
        isolation_rule: &TestIsolationRule,
        variety_rule: &InputVarietyRule,
    ) -> ScoreBreakdown {
        use super::rules::AnalysisRule;

        ScoreBreakdown {
            assertion_quality: assertion_rule.calculate_score(tests, issues),
            error_coverage: error_rule.calculate_score(tests, issues),
            boundary_conditions: boundary_rule.calculate_score(tests, issues),
            test_isolation: isolation_rule.calculate_score(tests, issues),
            input_variety: variety_rule.calculate_score(tests, issues),
        }
    }

    /// Get a description of the grade
    pub fn grade_description(grade: Grade) -> &'static str {
        match grade {
            Grade::A => "Excellent - Tests are well-structured with strong assertions",
            Grade::B => "Good - Tests are solid but have room for improvement",
            Grade::C => "Fair - Tests provide basic coverage but need strengthening",
            Grade::D => "Poor - Tests have significant quality issues",
            Grade::F => "Failing - Tests need major improvements",
        }
    }

    /// Get recommendations based on breakdown scores
    pub fn recommendations(breakdown: &ScoreBreakdown) -> Vec<String> {
        let mut recs = Vec::new();

        if breakdown.assertion_quality < 15 {
            recs.push(
                "Focus on using stronger assertions like toBe() and toEqual() with specific values"
                    .to_string(),
            );
        }

        if breakdown.error_coverage < 15 {
            recs.push(
                "Add tests for error conditions using toThrow() or rejects.toThrow()".to_string(),
            );
        }

        if breakdown.boundary_conditions < 15 {
            recs.push("Test edge cases and boundary values (0, empty, min/max)".to_string());
        }

        if breakdown.test_isolation < 15 {
            recs.push("Ensure tests are isolated - use beforeEach to reset state".to_string());
        }

        if breakdown.input_variety < 15 {
            recs.push(
                "Vary test inputs - include edge cases like null, empty, negative".to_string(),
            );
        }

        if recs.is_empty() {
            recs.push("Tests are in good shape! Consider adding more edge cases.".to_string());
        }

        recs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_score_calculation() {
        let breakdown = ScoreBreakdown {
            assertion_quality: 20,
            error_coverage: 18,
            boundary_conditions: 15,
            test_isolation: 22,
            input_variety: 20,
        };

        let score = ScoreCalculator::calculate(&breakdown);
        // total() normalizes sum (max 125) to 0-100: (95 * 100) / 125 = 76
        assert_eq!(score.value, 76);
        assert_eq!(score.grade, Grade::C);
    }

    #[test]
    fn test_grade_from_score() {
        assert_eq!(Grade::from_score(100), Grade::A);
        assert_eq!(Grade::from_score(90), Grade::A);
        assert_eq!(Grade::from_score(89), Grade::B);
        assert_eq!(Grade::from_score(80), Grade::B);
        assert_eq!(Grade::from_score(79), Grade::C);
        assert_eq!(Grade::from_score(70), Grade::C);
        assert_eq!(Grade::from_score(69), Grade::D);
        assert_eq!(Grade::from_score(60), Grade::D);
        assert_eq!(Grade::from_score(59), Grade::F);
        assert_eq!(Grade::from_score(0), Grade::F);
        assert_eq!(Grade::from_score(95), Grade::A);
        assert_eq!(Grade::from_score(85), Grade::B);
        assert_eq!(Grade::from_score(75), Grade::C);
        assert_eq!(Grade::from_score(65), Grade::D);
        assert_eq!(Grade::from_score(55), Grade::F);
    }
}
