//! Mutation-resistant assertion detection
//!
//! Flags assertions that might let mutants survive (e.g. toBeGreaterThan(0) instead of toBe(3)).

use super::AnalysisRule;
use crate::{AssertionKind, Issue, Rule, Severity, TestCase};
use tree_sitter::Tree;

/// Rule that flags assertions which are not specific enough to kill common mutants.
pub struct MutationResistantRule;

impl MutationResistantRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MutationResistantRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for MutationResistantRule {
    fn name(&self) -> &'static str {
        "mutation-resistant"
    }

    fn analyze(&self, tests: &[TestCase], _source: &str, _tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();
        let raw_lower = |raw: &str| raw.to_lowercase();

        for test in tests {
            for assertion in &test.assertions {
                let (flag, suggestion) = match &assertion.kind {
                    AssertionKind::ToBeGreaterThan => {
                        let r = raw_lower(&assertion.raw);
                        // expect(x).toBeGreaterThan(0) or toBeGreaterThanOrEqual(0) lets any positive value pass
                        if r.contains("tobegreaterthan(0)")
                            || r.contains("tobegreaterthanorequal(0)")
                            || r.contains(">= 0")
                            || r.contains("> 0")
                        {
                            (
                                true,
                                "Replace with exact value: expect(result).toBe(expected) so mutations (e.g. off-by-one) are caught".to_string(),
                            )
                        } else {
                            (false, String::new())
                        }
                    }
                    AssertionKind::ToBeLessThan => {
                        let r = raw_lower(&assertion.raw);
                        // expect(x).toBeLessThanOrEqual(max) or toBeLessThan(1) allows many values to pass
                        if r.contains("tobelessthanorequal(")
                            || r.contains("tobelessthan(1)")
                            || r.contains("< 1")
                        {
                            (
                                true,
                                "Replace with exact value: expect(result).toBe(expected) so mutations are caught".to_string(),
                            )
                        } else {
                            (false, String::new())
                        }
                    }
                    _ => (false, String::new()),
                };
                if flag {
                    issues.push(Issue {
                        rule: Rule::MutationResistant,
                        severity: Severity::Info,
                        message: format!(
                            "Assertion in '{}' may let mutants survive: {}",
                            test.name,
                            if assertion.raw.len() > 45 {
                                format!("{}...", &assertion.raw[..42])
                            } else {
                                assertion.raw.clone()
                            }
                        ),
                        location: assertion.location.clone(),
                        suggestion: Some(suggestion),
                        fix: None,
                    });
                }
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
    use crate::{Assertion, AssertionKind, Location, TestCase};

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
    fn positive_detects_to_be_greater_than_zero() {
        let rule = MutationResistantRule::new();
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("test")
            .unwrap();
        let tests = vec![make_test(
            "count is positive",
            vec![make_assertion(
                AssertionKind::ToBeGreaterThan,
                "expect(x > 0).toBe(true)",
            )],
        )];
        let issues = rule.analyze(&tests, "", &tree);
        assert!(!issues.is_empty());
        assert!(issues.iter().any(|i| i.rule == Rule::MutationResistant));
    }

    #[test]
    fn negative_exact_value_no_issue() {
        let rule = MutationResistantRule::new();
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("test")
            .unwrap();
        let tests = vec![make_test(
            "returns three",
            vec![make_assertion(
                AssertionKind::ToBe,
                "expect(result).toBe(3)",
            )],
        )];
        let issues = rule.analyze(&tests, "", &tree);
        assert!(issues.is_empty());
    }

    #[test]
    fn score_returns_25() {
        let rule = MutationResistantRule::new();
        let tests: Vec<TestCase> = vec![];
        let issues: Vec<Issue> = vec![];
        assert_eq!(rule.calculate_score(&tests, &issues), 25);
    }
}
