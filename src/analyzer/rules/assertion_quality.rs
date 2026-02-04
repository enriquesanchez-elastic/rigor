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
                });
            }
        }

        issues
    }

    fn calculate_score(&self, tests: &[TestCase], issues: &[Issue]) -> u8 {
        if tests.is_empty() {
            return 0;
        }

        let mut score: i32 = 25;

        // Count assertion quality issues
        let weak_assertions = issues
            .iter()
            .filter(|i| i.rule == Rule::WeakAssertion)
            .count();
        let no_assertions = issues
            .iter()
            .filter(|i| i.rule == Rule::NoAssertions)
            .count();

        // Deduct points for weak assertions (-2 each, max -10)
        score -= (weak_assertions as i32 * 2).min(10);

        // Deduct heavily for tests without assertions (-5 each, max -15)
        score -= (no_assertions as i32 * 5).min(15);

        // Snapshot overuse
        let snapshot_overuse = issues
            .iter()
            .filter(|i| i.rule == Rule::SnapshotOveruse)
            .count();
        score -= (snapshot_overuse as i32 * 3).min(9);

        // Calculate strong assertion ratio bonus
        let total_assertions: usize = tests.iter().map(|t| t.assertions.len()).sum();
        let strong_assertions = tests
            .iter()
            .flat_map(|t| &t.assertions)
            .filter(|a| a.quality == AssertionQuality::Strong)
            .count();

        if total_assertions > 0 {
            let strong_ratio = strong_assertions as f32 / total_assertions as f32;
            // Bonus for high ratio of strong assertions (up to +5)
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
}
