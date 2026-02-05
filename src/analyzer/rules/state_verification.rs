//! State verification rule
//!
//! Flags tests that only verify return values but don't verify state changes or side effects.

use super::AnalysisRule;
use crate::{AssertionKind, Issue, Rule, Severity, TestCase};
use tree_sitter::Tree;

/// Rule that suggests verifying state when testing functions with side effects.
pub struct StateVerificationRule;

impl StateVerificationRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for StateVerificationRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for StateVerificationRule {
    fn name(&self) -> &'static str {
        "state-verification"
    }

    fn analyze(&self, tests: &[TestCase], _source: &str, _tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();

        for test in tests {
            let has_mock_or_state_assertion = test.assertions.iter().any(|a| {
                matches!(
                    a.kind,
                    AssertionKind::ToHaveBeenCalled
                        | AssertionKind::ToHaveBeenCalledTimes
                        | AssertionKind::ToHaveBeenNthCalledWith
                        | AssertionKind::ToHaveProperty
                ) || a.raw.contains("toHaveBeenCalled")
                    || a.raw.contains("toHaveBeenNthCalledWith")
            });

            let only_return_value = test.assertions.len() == 1
                && matches!(
                    test.assertions[0].kind,
                    AssertionKind::ToBe
                        | AssertionKind::ToEqual
                        | AssertionKind::ToStrictEqual
                        | AssertionKind::ToBeTruthy
                        | AssertionKind::ToBeDefined
                );

            let name_lower = test.name.to_lowercase();
            let might_have_side_effects = name_lower.contains("update")
                || name_lower.contains("set")
                || name_lower.contains("add")
                || name_lower.contains("remove")
                || name_lower.contains("create")
                || name_lower.contains("save")
                || name_lower.contains("delete");

            if might_have_side_effects && only_return_value && !has_mock_or_state_assertion {
                issues.push(Issue {
                    rule: Rule::StateVerification,
                    severity: Severity::Info,
                    message: format!(
                        "Test '{}' may have side effects but only checks return value",
                        test.name
                    ),
                    location: test.location.clone(),
                    suggestion: Some(
                        "Verify state or mocks: expect(mockFn).toHaveBeenCalledWith(expected); or expect(state).toEqual(expected)".to_string(),
                    ),
                });
            }
        }

        issues
    }

    fn calculate_score(&self, _tests: &[TestCase], _issues: &[Issue]) -> u8 {
        25
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Assertion, AssertionKind, Issue, Location, TestCase};

    fn make_test(name: &str, assertions: Vec<Assertion>) -> TestCase {
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
    fn positive_detects_side_effect_name_with_only_return_assertion() {
        let rule = StateVerificationRule::new();
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("test")
            .unwrap();
        let tests = vec![make_test(
            "should update user",
            vec![make_assertion(
                AssertionKind::ToBe,
                "expect(result).toBe(true)",
            )],
        )];
        let issues = rule.analyze(&tests, "", &tree);
        assert!(!issues.is_empty());
        assert!(issues.iter().any(|i| i.rule == Rule::StateVerification));
    }

    #[test]
    fn negative_no_issue_when_no_side_effect_name() {
        let rule = StateVerificationRule::new();
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("test")
            .unwrap();
        let tests = vec![make_test(
            "returns sum",
            vec![make_assertion(
                AssertionKind::ToBe,
                "expect(add(1,2)).toBe(3)",
            )],
        )];
        let issues = rule.analyze(&tests, "", &tree);
        assert!(issues.is_empty());
    }

    #[test]
    fn score_returns_25() {
        let rule = StateVerificationRule::new();
        let tests: Vec<TestCase> = vec![];
        let issues: Vec<Issue> = vec![];
        assert_eq!(rule.calculate_score(&tests, &issues), 25);
    }
}
