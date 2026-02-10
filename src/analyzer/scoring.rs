//! Score calculation for test quality
//!
//! ## Scoring model: No double-counting
//! Each issue affects exactly one of:
//! - **Category score** (via the six category rules' `calculate_score`): WeakAssertion, NoAssertions,
//!   MissingErrorTest, MissingBoundaryTest, SharedState, HardcodedValues, LimitedInputVariety,
//!   DuplicateTest, TrivialAssertion, AssertionIntentMismatch, MutationResistant, BoundarySpecificity,
//!   StateVerification, ReturnPathCoverage, BehavioralCompleteness, SideEffectNotVerified.
//! - **Penalty only** (not category): DebugCode, FocusedTest, SkippedTest, EmptyTest, FlakyPattern,
//!   MockAbuse, SnapshotOveruse, VagueTestName, MissingAwait, RtlPreferScreen, RtlPreferSemantic,
//!   RtlPreferUserEvent. See `crate::rule_scoring_category` for the mapping.

use crate::{
    rule_scoring_category, CategoryBreakdownEntry, Grade, Issue, Rule, Score, ScoreBreakdown,
    ScoringWeights, Severity, TestCase, TestType, TransparentBreakdown,
};

use super::rules::{
    AiSmellsRule, AssertionQualityRule, BoundaryConditionsRule, ErrorCoverageRule,
    InputVarietyRule, TestIsolationRule,
};

/// Penalty points per issue by severity (applied after category score).
///
/// These penalties apply only to "penalty-only" issues — rules whose impact is NOT already
/// captured by a category score (e.g. DebugCode, FocusedTest, FlakyPattern, VagueTestName).
/// Category-affecting rules (WeakAssertion, MissingErrorTest, etc.) reduce the category
/// score directly and are NOT double-counted here.
///
/// ## Rationale for values
/// - **PENALTY_PER_ERROR (7):** Errors like focused tests or debug code in CI are serious
///   code hygiene issues that should noticeably drop the score.
/// - **PENALTY_PER_WARNING (3):** Warnings like vague test names hurt readability but are
///   less severe than errors.
/// - **PENALTY_PER_INFO (1):** Informational suggestions are low-severity nudges.
///
/// ## Rationale for caps
/// - **MAX_PENALTY_FROM_ERRORS (50):** Beyond ~7 errors the file is already an F; further
///   penalties don't provide signal but the cap prevents a single category from dominating.
/// - **MAX_PENALTY_FROM_WARNINGS (40):** Generous cap to ensure files with many warnings
///   (e.g. 15+ vague test names) are penalized substantially.
/// - **MAX_PENALTY_FROM_INFO (15):** Info issues are low-severity; cap prevents them from
///   overwhelming the score.
///
/// Total maximum penalty: 105 (enough to bring a perfect 100 well below zero, clamped to 0).
const PENALTY_PER_ERROR: i32 = 7;
const PENALTY_PER_WARNING: i32 = 3;
const PENALTY_PER_INFO: i32 = 1;
const MAX_PENALTY_FROM_ERRORS: i32 = 50;
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

    /// Apply issue-based penalty (no double-counting).
    /// Only issues that do NOT affect a category score (e.g. DebugCode, FocusedTest)
    /// are counted here. Issues that reduce a category (e.g. WeakAssertion, MissingErrorTest)
    /// have already been reflected in the category score and are not penalized again.
    pub fn apply_issue_penalty(score: Score, issues: &[Issue]) -> Score {
        let (errors, warnings, infos) = issues.iter().fold((0i32, 0i32, 0i32), |acc, i| {
            if rule_scoring_category(&i.rule).is_some() {
                acc
            } else {
                match i.severity {
                    Severity::Error => (acc.0 + 1, acc.1, acc.2),
                    Severity::Warning => (acc.0, acc.1 + 1, acc.2),
                    Severity::Info => (acc.0, acc.1, acc.2 + 1),
                }
            }
        });

        let penalty = (errors * PENALTY_PER_ERROR).min(MAX_PENALTY_FROM_ERRORS)
            + (warnings * PENALTY_PER_WARNING).min(MAX_PENALTY_FROM_WARNINGS)
            + (infos * PENALTY_PER_INFO).min(MAX_PENALTY_FROM_INFO);

        let value = (score.value as i32 - penalty).clamp(0, 100) as u8;
        Score::new(value)
    }

    /// Calculate breakdown from tests and issues
    #[allow(clippy::too_many_arguments)]
    pub fn calculate_breakdown(
        tests: &[TestCase],
        issues: &[Issue],
        assertion_rule: &AssertionQualityRule,
        error_rule: &ErrorCoverageRule,
        boundary_rule: &BoundaryConditionsRule,
        isolation_rule: &TestIsolationRule,
        variety_rule: &InputVarietyRule,
        ai_smells_rule: &AiSmellsRule,
    ) -> ScoreBreakdown {
        use super::rules::AnalysisRule;

        ScoreBreakdown {
            assertion_quality: assertion_rule.calculate_score(tests, issues),
            error_coverage: error_rule.calculate_score(tests, issues),
            boundary_conditions: boundary_rule.calculate_score(tests, issues),
            test_isolation: isolation_rule.calculate_score(tests, issues),
            input_variety: variety_rule.calculate_score(tests, issues),
            ai_smells: ai_smells_rule.calculate_score(tests, issues),
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

    /// Build full transparent breakdown: category scores, weights, and penalties.
    /// Penalty counts only issues that do not affect a category (no double-counting).
    pub fn build_transparent_breakdown(
        breakdown: &ScoreBreakdown,
        issues: &[Issue],
        test_type: TestType,
    ) -> TransparentBreakdown {
        let weights = ScoringWeights::for_test_type(test_type);
        let categories = [
            (
                "Assertion Quality",
                breakdown.assertion_quality,
                weights.assertion_quality,
            ),
            (
                "Error Coverage",
                breakdown.error_coverage,
                weights.error_coverage,
            ),
            (
                "Boundary Conditions",
                breakdown.boundary_conditions,
                weights.boundary_conditions,
            ),
            (
                "Test Isolation",
                breakdown.test_isolation,
                weights.test_isolation,
            ),
            (
                "Input Variety",
                breakdown.input_variety,
                weights.input_variety,
            ),
            ("AI Smells", breakdown.ai_smells, weights.ai_smells),
        ];

        let category_entries: Vec<CategoryBreakdownEntry> = categories
            .iter()
            .map(|(name, raw, weight_pct)| {
                let weighted_contribution =
                    ((*raw as u32) * (*weight_pct as u32) / 25).min(100) as u8;
                CategoryBreakdownEntry {
                    category_name: (*name).to_string(),
                    raw_score: *raw,
                    max_raw: 25,
                    weight_pct: *weight_pct,
                    weighted_contribution,
                }
            })
            .collect();

        let total_before_penalties = weights.calculate_total(breakdown);

        // Only count penalty-only issues (not category issues) to avoid double-counting
        let (errors, warnings, infos) = issues.iter().fold((0i32, 0i32, 0i32), |acc, i| {
            if rule_scoring_category(&i.rule).is_some() {
                acc
            } else {
                match i.severity {
                    Severity::Error => (acc.0 + 1, acc.1, acc.2),
                    Severity::Warning => (acc.0, acc.1 + 1, acc.2),
                    Severity::Info => (acc.0, acc.1, acc.2 + 1),
                }
            }
        });

        let penalty_from_errors = (errors * PENALTY_PER_ERROR).min(MAX_PENALTY_FROM_ERRORS);
        let penalty_from_warnings = (warnings * PENALTY_PER_WARNING).min(MAX_PENALTY_FROM_WARNINGS);
        let penalty_from_info = (infos * PENALTY_PER_INFO).min(MAX_PENALTY_FROM_INFO);
        let penalty_total = penalty_from_errors + penalty_from_warnings + penalty_from_info;

        let final_score = (total_before_penalties as i32 - penalty_total).clamp(0, 100) as u8;

        TransparentBreakdown {
            categories: category_entries,
            total_before_penalties,
            penalty_total,
            penalty_from_errors,
            penalty_from_warnings,
            penalty_from_info,
            final_score,
            per_test_aggregated: None, // Set by engine when per-test aggregation changes the score
        }
    }

    /// Get recommendations based on breakdown scores, issues, and grade.
    /// Uses category thresholds of 20 and adds issue-driven recommendations.
    pub fn recommendations(
        breakdown: &ScoreBreakdown,
        issues: &[Issue],
        grade: Grade,
    ) -> Vec<String> {
        let mut recs = Vec::new();

        // Category-based (threshold 20)
        if breakdown.assertion_quality < 20 {
            recs.push(
                "Focus on using stronger assertions like toBe() and toEqual() with specific values"
                    .to_string(),
            );
        }
        if breakdown.error_coverage < 20 {
            recs.push(
                "Add tests for error conditions using toThrow() or rejects.toThrow()".to_string(),
            );
        }
        if breakdown.boundary_conditions < 20 {
            recs.push("Test edge cases and boundary values (0, empty, min/max)".to_string());
        }
        if breakdown.test_isolation < 20 {
            recs.push("Ensure tests are isolated - use beforeEach to reset state".to_string());
        }
        if breakdown.input_variety < 20 {
            recs.push(
                "Vary test inputs - include edge cases like null, empty, negative".to_string(),
            );
        }

        // Issue-driven recommendations
        let flaky_count = issues
            .iter()
            .filter(|i| i.rule == Rule::FlakyPattern)
            .count();
        if flaky_count >= 3 {
            recs.push(
                "Mock non-deterministic APIs (Date.now, Math.random) for reliable tests"
                    .to_string(),
            );
        }
        let rtl_count = issues
            .iter()
            .filter(|i| {
                i.rule == Rule::RtlPreferScreen
                    || i.rule == Rule::RtlPreferSemantic
                    || i.rule == Rule::RtlPreferUserEvent
            })
            .count();
        if rtl_count >= 2 {
            recs.push(
                "Use semantic queries (getByRole, getByLabelText) instead of getByTestId / querySelector"
                    .to_string(),
            );
        }
        let vague_count = issues
            .iter()
            .filter(|i| i.rule == Rule::VagueTestName)
            .count();
        if vague_count >= 2 {
            recs.push("Use descriptive test names that state expected behavior".to_string());
        }
        let focused_count = issues
            .iter()
            .filter(|i| i.rule == Rule::FocusedTest)
            .count();
        if focused_count >= 2 {
            recs.push("Remove .only / fit() so the full test suite runs".to_string());
        }
        if issues.iter().any(|i| i.rule == Rule::DebugCode) {
            recs.push("Remove debugger statements and console.log from tests".to_string());
        }

        // Fallback by grade
        if recs.is_empty() {
            if matches!(grade, Grade::A | Grade::B) {
                recs.push("Tests are in good shape! Consider adding more edge cases.".to_string());
            } else {
                recs.push("Address the warnings above to improve test quality.".to_string());
            }
        }

        recs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Issue, Location, Rule, Severity};

    #[test]
    fn test_score_calculation() {
        let breakdown = ScoreBreakdown {
            assertion_quality: 20,
            error_coverage: 18,
            boundary_conditions: 15,
            test_isolation: 22,
            input_variety: 20,
            ai_smells: 25,
        };

        let score = ScoreCalculator::calculate(&breakdown);
        // total() normalizes sum (max 150, six categories) to 0-100: (120 * 100) / 150 = 80
        assert_eq!(score.value, 80);
        assert_eq!(score.grade, Grade::B);
    }

    #[test]
    fn test_score_calculation_perfect() {
        let breakdown = ScoreBreakdown {
            assertion_quality: 25,
            error_coverage: 25,
            boundary_conditions: 25,
            test_isolation: 25,
            input_variety: 25,
            ai_smells: 25,
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
            ai_smells: 0,
        };
        let score = ScoreCalculator::calculate(&breakdown);
        assert_eq!(score.value, 0);
        assert_eq!(score.grade, Grade::F);
    }

    #[test]
    fn test_breakdown_total_normalization() {
        // Sum = 60, normalized = (60 * 100) / 150 = 40
        let breakdown = ScoreBreakdown {
            assertion_quality: 10,
            error_coverage: 10,
            boundary_conditions: 10,
            test_isolation: 10,
            input_variety: 10,
            ai_smells: 10,
        };
        assert_eq!(breakdown.total(), 40);

        // Sum = 150, normalized = (150 * 100) / 150 = 100
        let perfect = ScoreBreakdown {
            assertion_quality: 25,
            error_coverage: 25,
            boundary_conditions: 25,
            test_isolation: 25,
            input_variety: 25,
            ai_smells: 25,
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
    fn test_apply_issue_penalty_only_counts_non_category_issues() {
        // WeakAssertion is a category issue — should NOT be penalized again
        let category_issues = vec![Issue {
            rule: crate::Rule::WeakAssertion,
            severity: Severity::Error,
            message: "error1".to_string(),
            location: Location::new(1, 1),
            suggestion: None,
            fix: None,
        }];
        let result = ScoreCalculator::apply_issue_penalty(Score::new(90), &category_issues);
        assert_eq!(result.value, 90, "category issues should not add penalty");

        // DebugCode is penalty-only — should be penalized
        let penalty_issues = vec![Issue {
            rule: crate::Rule::DebugCode,
            severity: Severity::Error,
            message: "debug".to_string(),
            location: Location::new(1, 1),
            suggestion: None,
            fix: None,
        }];
        let result = ScoreCalculator::apply_issue_penalty(Score::new(90), &penalty_issues);
        // 1 error * 7 = 7 penalty, 90 - 7 = 83
        assert_eq!(result.value, 83, "penalty-only issues should reduce score");
    }

    #[test]
    fn test_apply_issue_penalty_clamped_to_zero() {
        let score = Score::new(10);
        // 8 penalty-only errors * 7 = 56, capped at 50, penalty 50 > score 10 → clamps to 0
        let issues: Vec<Issue> = (0..8)
            .map(|i| Issue {
                rule: crate::Rule::DebugCode,
                severity: Severity::Error,
                message: format!("err{}", i),
                location: Location::new(i + 1, 1),
                suggestion: None,
                fix: None,
            })
            .collect();
        let result = ScoreCalculator::apply_issue_penalty(score, &issues);
        assert_eq!(result.value, 0);
        assert_eq!(result.grade, Grade::F);
    }

    #[test]
    fn test_apply_issue_penalty_mixed_severities() {
        let score = Score::new(95);
        // Only penalty-only rules count: DebugCode, FocusedTest, VagueTestName
        let issues = vec![
            Issue {
                rule: crate::Rule::DebugCode,
                severity: Severity::Error,
                message: "e".to_string(),
                location: Location::new(1, 1),
                suggestion: None,
                fix: None,
            },
            Issue {
                rule: crate::Rule::VagueTestName,
                severity: Severity::Warning,
                message: "w".to_string(),
                location: Location::new(2, 1),
                suggestion: None,
                fix: None,
            },
            Issue {
                rule: crate::Rule::FocusedTest,
                severity: Severity::Info,
                message: "i".to_string(),
                location: Location::new(3, 1),
                suggestion: None,
                fix: None,
            },
        ];
        let result = ScoreCalculator::apply_issue_penalty(score, &issues);
        // 1*7 + 1*3 + 1*1 = 11 penalty, 95 - 11 = 84
        assert_eq!(result.value, 84);
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
            ai_smells: 10,
        };
        let recs = ScoreCalculator::recommendations(&breakdown, &[], Grade::F);
        assert!(recs.len() >= 5);
        assert!(recs.iter().any(|r| r.contains("assertion")));
        assert!(recs.iter().any(|r| r.contains("error")));
        assert!(recs.iter().any(|r| r.contains("edge cases")));
        assert!(recs.iter().any(|r| r.contains("isolated")));
        assert!(recs.iter().any(|r| r.contains("Vary")));
    }

    #[test]
    fn test_recommendations_high_scores() {
        let breakdown = ScoreBreakdown {
            assertion_quality: 20,
            error_coverage: 20,
            boundary_conditions: 20,
            test_isolation: 20,
            input_variety: 20,
            ai_smells: 20,
        };
        let recs = ScoreCalculator::recommendations(&breakdown, &[], Grade::A);
        assert_eq!(recs.len(), 1);
        assert!(recs[0].contains("good shape"));
    }

    #[test]
    fn test_recommendations_grade_fallback_below_b() {
        let breakdown = ScoreBreakdown {
            assertion_quality: 25,
            error_coverage: 25,
            boundary_conditions: 25,
            test_isolation: 25,
            input_variety: 25,
            ai_smells: 25,
        };
        let recs = ScoreCalculator::recommendations(&breakdown, &[], Grade::C);
        assert_eq!(recs.len(), 1);
        assert!(recs[0].contains("Address the warnings"));
    }

    #[test]
    fn test_grade_description_all_grades() {
        assert!(ScoreCalculator::grade_description(Grade::A).contains("Excellent"));
        assert!(ScoreCalculator::grade_description(Grade::B).contains("Good"));
        assert!(ScoreCalculator::grade_description(Grade::C).contains("Fair"));
        assert!(ScoreCalculator::grade_description(Grade::D).contains("Poor"));
        assert!(ScoreCalculator::grade_description(Grade::F).contains("Failing"));
    }

    #[test]
    fn test_build_transparent_breakdown_sums_to_final_score() {
        let breakdown = ScoreBreakdown {
            assertion_quality: 20,
            error_coverage: 18,
            boundary_conditions: 15,
            test_isolation: 17,
            input_variety: 15,
            ai_smells: 25,
        };
        let issues: Vec<Issue> = vec![
            Issue {
                rule: Rule::WeakAssertion,
                severity: Severity::Warning,
                message: "w".to_string(),
                location: Location::new(1, 1),
                suggestion: None,
                fix: None,
            },
            Issue {
                rule: Rule::DebugCode,
                severity: Severity::Error,
                message: "e".to_string(),
                location: Location::new(2, 1),
                suggestion: None,
                fix: None,
            },
        ];
        let tb = ScoreCalculator::build_transparent_breakdown(&breakdown, &issues, TestType::Unit);
        assert_eq!(tb.categories.len(), 6);
        assert_eq!(
            tb.final_score as i32,
            tb.total_before_penalties as i32 - tb.penalty_total
        );
        // Only DebugCode (penalty-only) should count; WeakAssertion is category-based
        assert!(
            tb.penalty_from_errors > 0,
            "DebugCode error should add penalty"
        );
        assert_eq!(
            tb.penalty_from_warnings, 0,
            "WeakAssertion warning is category-only, no penalty"
        );
    }

    #[test]
    fn test_no_penalty_for_category_issues() {
        let breakdown = ScoreBreakdown {
            assertion_quality: 20,
            error_coverage: 25,
            boundary_conditions: 25,
            test_isolation: 25,
            input_variety: 25,
            ai_smells: 25,
        };
        let issues = vec![Issue {
            rule: Rule::WeakAssertion,
            severity: Severity::Warning,
            message: "weak".to_string(),
            location: Location::new(1, 1),
            suggestion: None,
            fix: None,
        }];
        let score_before = ScoreCalculator::calculate_weighted(&breakdown, TestType::Unit);
        let score_after = ScoreCalculator::apply_issue_penalty(score_before, &issues);
        // Category-only issue should not add any penalty
        let tb = ScoreCalculator::build_transparent_breakdown(&breakdown, &issues, TestType::Unit);
        assert_eq!(tb.penalty_from_warnings, 0);
        assert_eq!(tb.penalty_total, 0);
        assert_eq!(score_after.value as i32, tb.final_score as i32);
    }

    #[test]
    fn test_penalty_for_penalty_only_issues() {
        let breakdown = ScoreBreakdown {
            assertion_quality: 25,
            error_coverage: 25,
            boundary_conditions: 25,
            test_isolation: 25,
            input_variety: 25,
            ai_smells: 25,
        };
        let issues = vec![
            Issue {
                rule: Rule::DebugCode,
                severity: Severity::Warning,
                message: "debug".to_string(),
                location: Location::new(1, 1),
                suggestion: None,
                fix: None,
            },
            Issue {
                rule: Rule::FocusedTest,
                severity: Severity::Error,
                message: "only".to_string(),
                location: Location::new(2, 1),
                suggestion: None,
                fix: None,
            },
        ];
        let score_before = ScoreCalculator::calculate_weighted(&breakdown, TestType::Unit);
        let score_after = ScoreCalculator::apply_issue_penalty(score_before, &issues);
        let tb = ScoreCalculator::build_transparent_breakdown(&breakdown, &issues, TestType::Unit);
        assert!(tb.penalty_total > 0);
        assert_eq!(score_after.value as i32, tb.final_score as i32);
    }
}
