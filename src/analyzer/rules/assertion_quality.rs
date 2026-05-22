//! Assertion quality analysis rule

use super::AnalysisRule;
use crate::{AssertionKind, AssertionQuality, Issue, Rule, Severity, TestCase};
use tree_sitter::Tree;

/// Rule for analyzing assertion quality
pub struct AssertionQualityRule;

impl AssertionQualityRule {
    pub fn new() -> Self {
        Self
    }

    fn suggestion_for_weak_assertion(method: &str) -> String {
        match method {
            m if m.contains("toBeDefined") => "Replace with: expect(result).toBe(expectedValue) or expect(result).toEqual({ id: 1, name: '...' })".to_string(),
            m if m.contains("toBeTruthy") => "Replace with: expect(result).toBe(true) or expect(result.success).toBe(true)".to_string(),
            m if m.contains("toBeFalsy") => "Replace with: expect(result).toBe(false) or expect(value).toBe(null)".to_string(),
            m if m.contains("toBeNull") => "If intentional, also add: expect(fn()).not.toBeNull(). Consider: expect(value).toBe(null) and expect(value).toEqual(expected)".to_string(),
            m if m.contains("toBeUndefined") => "Replace with: expect(result).toBeDefined() and expect(result).toEqual(specificValue)".to_string(),
            _ => "Replace with: expect(actual).toBe(expected) or expect(actual).toEqual(expected)".to_string(),
        }
    }
}

impl Default for AssertionQualityRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for AssertionQualityRule {
    fn name(&self) -> &'static str {
        "assertion-quality"
    }

    fn analyze(&self, tests: &[TestCase], _source: &str, _tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();

        for test in tests {
            // Check for tests with no assertions
            if test.assertions.is_empty() && !test.is_skipped {
                issues.push(Issue {
                    rule: Rule::NoAssertions,
                    severity: Severity::Error,
                    message: format!("Test '{}' has no assertions", test.name),
                    location: test.location.clone(),
                    suggestion: Some(
                        "Add assertions, e.g. expect(result).toBe(expected) or expect(fn).toHaveBeenCalledWith(arg)".to_string(),
                    ),
                    fix: None,
                });
                continue;
            }

            // Check assertion quality
            for assertion in &test.assertions {
                if assertion.quality == AssertionQuality::Weak {
                    issues.push(Issue {
                        rule: Rule::WeakAssertion,
                        severity: Severity::Warning,
                        message: format!(
                            "Weak assertion in test '{}': {}",
                            test.name,
                            Self::describe_assertion(&assertion.raw)
                        ),
                        location: assertion.location.clone(),
                        suggestion: Some(Self::suggestion_for_weak_assertion(&assertion.raw)),
                        fix: None,
                    });
                }
            }

            // Check for skipped tests
            if test.is_skipped {
                issues.push(Issue {
                    rule: Rule::SkippedTest,
                    severity: Severity::Info,
                    message: format!("Test '{}' is skipped", test.name),
                    location: test.location.clone(),
                    suggestion: Some(
                        "Remove .skip or .todo if the test should be active".to_string(),
                    ),
                    fix: None,
                });
            }

            // Snapshot-only test: no specific assertions
            let snapshot_count = test
                .assertions
                .iter()
                .filter(|a| {
                    matches!(
                        a.kind,
                        AssertionKind::ToMatchSnapshot | AssertionKind::ToMatchInlineSnapshot
                    )
                })
                .count();
            let total_in_test = test.assertions.len();
            if total_in_test > 0 && snapshot_count == total_in_test {
                issues.push(Issue {
                    rule: Rule::SnapshotOveruse,
                    severity: Severity::Warning,
                    message: format!(
                        "Test '{}' uses only snapshot assertions - add specific value checks",
                        test.name
                    ),
                    location: test.location.clone(),
                    suggestion: Some(
                        "Add specific checks: expect(obj).toMatchSnapshot(); expect(obj.items).toHaveLength(3); expect(obj.status).toBe('ok')".to_string(),
                    ),
                    fix: None,
                });
            }
        }

        // File-level: >50% snapshot assertions
        let total_assertions: usize = tests.iter().map(|t| t.assertions.len()).sum();
        let snapshot_assertions: usize = tests
            .iter()
            .flat_map(|t| &t.assertions)
            .filter(|a| {
                matches!(
                    a.kind,
                    AssertionKind::ToMatchSnapshot | AssertionKind::ToMatchInlineSnapshot
                )
            })
            .count();
        if total_assertions > 0 {
            let ratio = snapshot_assertions as f32 / total_assertions as f32;
            if ratio > 0.5 {
                issues.push(Issue {
                    rule: Rule::SnapshotOveruse,
                    severity: Severity::Warning,
                    message: format!(
                        "Over half of assertions ({}/{}) are snapshots - consider more specific assertions",
                        snapshot_assertions, total_assertions
                    ),
                    location: crate::Location::new(1, 1),
                    suggestion: Some(
                        "Prefer toBe(), toEqual(), or toHaveLength() for critical behavior; use snapshots sparingly".to_string(),
                    ),
                    fix: None,
                });
            }
        }

        issues
    }

    fn calculate_score(&self, tests: &[TestCase], issues: &[Issue]) -> u8 {
        if tests.is_empty() {
            return 0;
        }

        let total_assertions = tests.iter().map(|t| t.assertions.len()).sum::<usize>().max(1);
        let total_tests = tests.len().max(1);
        let mut score: i32 = 25;

        let weak = issues.iter().filter(|i| i.rule == Rule::WeakAssertion).count();
        let no_assert = issues.iter().filter(|i| i.rule == Rule::NoAssertions).count();
        let snap = issues.iter().filter(|i| i.rule == Rule::SnapshotOveruse).count();
        let trivial = issues.iter().filter(|i| i.rule == Rule::TrivialAssertion).count();

        // Ratio-based: proportion of assertions/tests affected
        score -= ((weak as f32 / total_assertions as f32).min(1.0) * 18.0) as i32;
        score -= ((no_assert as f32 / total_tests as f32).min(1.0) * 24.0) as i32;
        score -= ((snap as f32 / total_tests as f32).min(1.0) * 12.0) as i32;
        score -= ((trivial as f32 / total_assertions as f32).min(1.0) * 12.0) as i32;

        // Phase 2 rules mapped to "Assertion Quality" category (see rule_scoring_category in lib.rs)
        let phase2 = issues
            .iter()
            .filter(|i| {
                matches!(
                    i.rule,
                    Rule::AssertionIntentMismatch
                        | Rule::MutationResistant
                        | Rule::BoundarySpecificity
                        | Rule::StateVerification
                        | Rule::BehavioralCompleteness
                        | Rule::SideEffectNotVerified
                )
            })
            .count();
        score -= ((phase2 as f32 / total_tests as f32).min(1.0) * 12.0) as i32;

        // Bonus: high strong-assertion ratio
        let strong = tests
            .iter()
            .flat_map(|t| &t.assertions)
            .filter(|a| a.quality == crate::AssertionQuality::Strong)
            .count();
        if total_assertions > 0 {
            let strong_ratio = strong as f32 / total_assertions as f32;
            if strong_ratio > 0.8 {
                score += 5;
            } else if strong_ratio > 0.6 {
                score += 3;
            }
        }

        score.clamp(0, 25) as u8
    }
}

impl AssertionQualityRule {
    fn describe_assertion(raw: &str) -> String {
        // Truncate long assertions
        if raw.len() > 50 {
            format!("{}...", &raw[..47])
        } else {
            raw.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Assertion, AssertionKind, Location};

    fn make_test_with_assertions(name: &str, assertions: Vec<Assertion>) -> TestCase {
        TestCase {
            name: name.to_string(),
            location: Location::new(1, 1),
            is_async: false,
            is_skipped: false,
            assertions,
            describe_block: None,
        }
    }

    fn make_assertion(kind: AssertionKind, raw: &str) -> Assertion {
        Assertion {
            kind: kind.clone(),
            quality: kind.quality(),
            location: Location::new(1, 1),
            raw: raw.to_string(),
        }
    }

    #[test]
    fn test_detect_weak_assertion() {
        let tests = vec![make_test_with_assertions(
            "weak test",
            vec![make_assertion(
                AssertionKind::ToBeDefined,
                "expect(x).toBeDefined()",
            )],
        )];

        let rule = AssertionQualityRule::new();
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("test")
            .unwrap();
        let issues = rule.analyze(&tests, "", &tree);

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule, Rule::WeakAssertion);
    }

    #[test]
    fn test_no_issues_for_strong_assertions() {
        let tests = vec![make_test_with_assertions(
            "strong test",
            vec![make_assertion(AssertionKind::ToBe, "expect(x).toBe(1)")],
        )];

        let rule = AssertionQualityRule::new();
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("test")
            .unwrap();
        let issues = rule.analyze(&tests, "", &tree);

        assert!(issues.is_empty());
    }

    #[test]
    fn test_detect_no_assertions() {
        let tests = vec![make_test_with_assertions("empty test", vec![])];

        let rule = AssertionQualityRule::new();
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("test")
            .unwrap();
        let issues = rule.analyze(&tests, "", &tree);

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule, Rule::NoAssertions);
    }

    #[test]
    fn one_weak_assertion_in_large_file_scores_higher_than_in_tiny_file() {
        use crate::{Issue, Location, Rule, Severity};
        let rule = AssertionQualityRule::new();

        // Build 10 weak issues (enough to hit the absolute cap in the old logic)
        let make_weak_issue = || Issue {
            rule: Rule::WeakAssertion,
            severity: Severity::Warning,
            message: "weak".to_string(),
            location: Location::new(1, 1),
            suggestion: None,
            fix: None,
        };
        let ten_weak_issues: Vec<Issue> = (0..10).map(|_| make_weak_issue()).collect();

        // Small file: 10 tests all with weak assertions (100% bad, 0 strong)
        let small_tests: Vec<TestCase> = (0..10)
            .map(|i| {
                make_test_with_assertions(
                    &format!("s{i}"),
                    vec![make_assertion(
                        crate::AssertionKind::ToBeDefined,
                        "expect(x).toBeDefined()",
                    )],
                )
            })
            .collect();

        // Large file: 10 weak + 90 moderate (ToHaveLength) assertions.
        // Moderate is NOT penalised as weak AND does NOT count as strong,
        // so the strong-ratio bonus is 0 — removing the accidental compensation
        // that would otherwise mask the absolute-count bug.
        let weak_assert =
            make_assertion(crate::AssertionKind::ToBeDefined, "expect(x).toBeDefined()");
        let moderate_assert =
            make_assertion(crate::AssertionKind::ToHaveLength, "expect(x).toHaveLength(3)");
        let large_tests: Vec<TestCase> = (0..10)
            .map(|i| make_test_with_assertions(&format!("w{i}"), vec![weak_assert.clone()]))
            .chain(
                (0..90)
                    .map(|i| make_test_with_assertions(&format!("t{i}"), vec![moderate_assert.clone()])),
            )
            .collect();

        let score_small = rule.calculate_score(&small_tests, &ten_weak_issues);
        let score_large = rule.calculate_score(&large_tests, &ten_weak_issues);

        assert!(
            score_large > score_small,
            "10 weak assertions in 100-test file ({score_large}) must score higher \
             than 10 weak assertions in 10-test file ({score_small})"
        );
    }

    #[test]
    fn phase2_assertion_quality_issues_reduce_score() {
        use crate::{Issue, Location, Rule, Severity};
        let rule = AssertionQualityRule::new();
        // Use a weak assertion so no strong-ratio bonus offsets the deduction.
        let tests = vec![make_test_with_assertions(
            "some test",
            vec![make_assertion(
                crate::AssertionKind::ToBeDefined,
                "expect(x).toBeDefined()",
            )],
        )];

        let no_issues: Vec<Issue> = vec![];
        let with_issues: Vec<Issue> = vec![
            Issue {
                rule: Rule::MutationResistant,
                severity: Severity::Info,
                message: "mutation".to_string(),
                location: Location::new(1, 1),
                suggestion: None,
                fix: None,
            },
            Issue {
                rule: Rule::BehavioralCompleteness,
                severity: Severity::Warning,
                message: "completeness".to_string(),
                location: Location::new(2, 1),
                suggestion: None,
                fix: None,
            },
        ];

        let score_clean = rule.calculate_score(&tests, &no_issues);
        let score_with = rule.calculate_score(&tests, &with_issues);

        assert!(
            score_with < score_clean,
            "Phase 2 assertion-quality issues must reduce assertion quality score \
             (clean={score_clean}, with_issues={score_with})"
        );
    }
}
