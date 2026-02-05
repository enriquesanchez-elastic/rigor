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
    use crate::Location;

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
    fn test_score_calculation_perfect() {
        let breakdown = ScoreBreakdown {
            assertion_quality: 25,
            error_coverage: 25,
            boundary_conditions: 25,
            test_isolation: 25,
            input_variety: 25,
        };
        let score = ScoreCalculator::calculate(&breakdown);
        assert_eq!(score.value, 100);
        assert_eq!(score.grade, Grade::A);
    }

    #[test]
    fn test_score_calculation_zero() {
        let breakdown = ScoreBreakdown {
            assertion_quality: 0,
            error_coverage: 0,
            boundary_conditions: 0,
            test_isolation: 0,
            input_variety: 0,
        };
        let score = ScoreCalculator::calculate(&breakdown);
        assert_eq!(score.value, 0);
        assert_eq!(score.grade, Grade::F);
    }

    #[test]
    fn test_breakdown_total_normalization() {
        // Sum = 50, normalized = (50 * 100) / 125 = 40
        let breakdown = ScoreBreakdown {
            assertion_quality: 10,
            error_coverage: 10,
            boundary_conditions: 10,
            test_isolation: 10,
            input_variety: 10,
        };
        assert_eq!(breakdown.total(), 40);

        // Sum = 125, normalized = (125 * 100) / 125 = 100
        let perfect = ScoreBreakdown {
            assertion_quality: 25,
            error_coverage: 25,
            boundary_conditions: 25,
            test_isolation: 25,
            input_variety: 25,
        };
        assert_eq!(perfect.total(), 100);
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

    #[test]
    fn test_apply_issue_penalty_no_issues() {
        let score = Score::new(90);
        let result = ScoreCalculator::apply_issue_penalty(score, &[]);
        assert_eq!(result.value, 90);
        assert_eq!(result.grade, Grade::A);
    }

    #[test]
    fn test_apply_issue_penalty_errors_reduce_score() {
        let score = Score::new(90);
        let issues = vec![
            Issue {
                rule: crate::Rule::WeakAssertion,
                severity: Severity::Error,
                message: "error1".to_string(),
                location: Location::new(1, 1),
                suggestion: None,
            },
            Issue {
                rule: crate::Rule::WeakAssertion,
                severity: Severity::Error,
                message: "error2".to_string(),
                location: Location::new(2, 1),
                suggestion: None,
            },
        ];
        let result = ScoreCalculator::apply_issue_penalty(score, &issues);
        // 2 errors * 5 = 10 penalty, 90 - 10 = 80
        assert_eq!(result.value, 80);
        assert_eq!(result.grade, Grade::B);
    }

    #[test]
    fn test_apply_issue_penalty_clamped_to_zero() {
        let score = Score::new(10);
        // 7 errors * 5 = 35 (capped), penalty 35 > score 10 → clamps to 0
        let issues: Vec<Issue> = (0..7)
            .map(|i| Issue {
                rule: crate::Rule::NoAssertions,
                severity: Severity::Error,
                message: format!("err{}", i),
                location: Location::new(i + 1, 1),
                suggestion: None,
            })
            .collect();
        let result = ScoreCalculator::apply_issue_penalty(score, &issues);
        assert_eq!(result.value, 0);
        assert_eq!(result.grade, Grade::F);
    }

    #[test]
    fn test_apply_issue_penalty_mixed_severities() {
        let score = Score::new(95);
        let issues = vec![
            Issue {
                rule: crate::Rule::WeakAssertion,
                severity: Severity::Error,
                message: "e".to_string(),
                location: Location::new(1, 1),
                suggestion: None,
            },
            Issue {
                rule: crate::Rule::VagueTestName,
                severity: Severity::Warning,
                message: "w".to_string(),
                location: Location::new(2, 1),
                suggestion: None,
            },
            Issue {
                rule: crate::Rule::HardcodedValues,
                severity: Severity::Info,
                message: "i".to_string(),
                location: Location::new(3, 1),
                suggestion: None,
            },
        ];
        let result = ScoreCalculator::apply_issue_penalty(score, &issues);
        // 1*5 + 1*2 + 1*1 = 8 penalty, 95 - 8 = 87
        assert_eq!(result.value, 87);
        assert_eq!(result.grade, Grade::B);
    }

    #[test]
    fn test_recommendations_low_scores() {
        let breakdown = ScoreBreakdown {
            assertion_quality: 10,
            error_coverage: 10,
            boundary_conditions: 10,
            test_isolation: 10,
            input_variety: 10,
        };
        let recs = ScoreCalculator::recommendations(&breakdown);
        assert_eq!(recs.len(), 5);
        assert!(recs[0].contains("assertion"));
        assert!(recs[1].contains("error"));
        assert!(recs[2].contains("edge cases"));
        assert!(recs[3].contains("isolated"));
        assert!(recs[4].contains("Vary"));
    }

    #[test]
    fn test_recommendations_high_scores() {
        let breakdown = ScoreBreakdown {
            assertion_quality: 20,
            error_coverage: 20,
            boundary_conditions: 20,
            test_isolation: 20,
            input_variety: 20,
        };
        let recs = ScoreCalculator::recommendations(&breakdown);
        assert_eq!(recs.len(), 1);
        assert!(recs[0].contains("good shape"));
    }

    #[test]
    fn test_grade_description_all_grades() {
        assert!(ScoreCalculator::grade_description(Grade::A).contains("Excellent"));
        assert!(ScoreCalculator::grade_description(Grade::B).contains("Good"));
        assert!(ScoreCalculator::grade_description(Grade::C).contains("Fair"));
        assert!(ScoreCalculator::grade_description(Grade::D).contains("Poor"));
        assert!(ScoreCalculator::grade_description(Grade::F).contains("Failing"));
    }
}
