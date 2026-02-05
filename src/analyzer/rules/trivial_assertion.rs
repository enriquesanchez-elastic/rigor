//! Trivial / nonsensical assertions: tests that don't meaningfully verify behavior.
//!
//! Flags assertions that always pass regardless of the code under test, e.g.
//! expect(true).toBe(true), expect(1).toBe(1), or expect(constant).toEqual(constant).
//! Such tests add noise and a false sense of coverage.

use super::AnalysisRule;
use crate::{Issue, Rule, Severity, TestCase};
use regex::Regex;
use tree_sitter::Tree;

/// Rule that detects trivial or nonsensical assertions.
pub struct TrivialAssertionRule;

/// Patterns for normalized assertion (no whitespace) — expect(lit).toX(lit)
fn trivial_patterns() -> Vec<Regex> {
    [
        r"expect\(true\)\.to(Be|Equal|StrictEqual)\(true\)",
        r"expect\(false\)\.to(Be|Equal|StrictEqual)\(false\)",
        r"expect\(1\)\.to(Be|Equal|StrictEqual)\(1\)",
        r"expect\(0\)\.to(Be|Equal|StrictEqual)\(0\)",
        r"expect\(null\)\.to(Be|Equal|StrictEqual)\(null\)",
        r"expect\(undefined\)\.to(Be|Equal|StrictEqual)\(undefined\)",
    ]
    .iter()
    .map(|s| Regex::new(s).unwrap())
    .collect()
}

impl TrivialAssertionRule {
    pub fn new() -> Self {
        Self
    }

    /// Raw assertion looks like expect(literal).toBe(literal) with same value.
    fn is_trivial_literal(raw: &str) -> bool {
        let normalized: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
        trivial_patterns().iter().any(|re| re.is_match(&normalized))
    }

    /// expect(1).toBe(1) style — same number both sides (regex backref may not match 1/1)
    fn same_number_both_sides(raw: &str) -> bool {
        let n: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
        let n_lower = n.to_lowercase();
        n_lower.contains("expect(1).tobe(1)")
            || n_lower.contains("expect(0).tobe(0)")
            || n_lower.contains("expect(1).toequal(1)")
            || n_lower.contains("expect(0).toequal(0)")
            || n_lower.contains("expect(true).tobe(true)")
            || n_lower.contains("expect(false).tobe(false)")
    }
}

impl Default for TrivialAssertionRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for TrivialAssertionRule {
    fn name(&self) -> &'static str {
        "trivial-assertion"
    }

    fn analyze(&self, tests: &[TestCase], _source: &str, _tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();

        for test in tests {
            if test.is_skipped {
                continue;
            }

            let mut trivial_count = 0;
            for assertion in &test.assertions {
                let raw = assertion.raw.trim();
                if raw.is_empty() {
                    continue;
                }

                if Self::is_trivial_literal(raw) || Self::same_number_both_sides(raw) {
                    trivial_count += 1;
                    issues.push(Issue {
                        rule: Rule::TrivialAssertion,
                        severity: Severity::Warning,
                        message: format!(
                            "Trivial assertion in '{}': always passes and does not verify behavior — {}",
                            test.name,
                            if raw.len() > 50 { format!("{}...", &raw[..47]) } else { raw.to_string() }
                        ),
                        location: assertion.location.clone(),
                        suggestion: Some(
                            "Assert on the actual result of the code under test (e.g. expect(actualResult).toBe(expected)) instead of literals.".to_string(),
                        ),
                    });
                }
            }

            // If every assertion in the test is trivial, add a test-level summary (Error)
            let total = test.assertions.len();
            if total > 0 && trivial_count == total {
                issues.push(Issue {
                    rule: Rule::TrivialAssertion,
                    severity: Severity::Error,
                    message: format!(
                        "Test '{}' only has trivial assertions — it does not test any real behavior",
                        test.name
                    ),
                    location: test.location.clone(),
                    suggestion: Some(
                        "Replace with assertions on the result of the code under test (e.g. expect(myFunction()).toBe(expected)).".to_string(),
                    ),
                });
            }
        }

        issues
    }

    fn calculate_score(&self, tests: &[TestCase], issues: &[Issue]) -> u8 {
        let trivial_count = issues
            .iter()
            .filter(|i| i.rule == Rule::TrivialAssertion)
            .count();
        if tests.is_empty() {
            return 25;
        }
        let deduction = (trivial_count as i32 * 2).min(15);
        (25 - deduction).max(0) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Assertion, AssertionKind, Location};

    fn test_case(name: &str, assertions: Vec<Assertion>) -> TestCase {
        TestCase {
            name: name.to_string(),
            location: Location::new(1, 1),
            is_async: false,
            is_skipped: false,
            assertions,
            describe_block: None,
        }
    }

    fn assertion(kind: AssertionKind, raw: &str) -> Assertion {
        let quality = kind.quality();
        Assertion {
            kind,
            quality,
            location: Location::new(1, 1),
            raw: raw.to_string(),
        }
    }

    #[test]
    fn flags_trivial_literal_assertion() {
        let tests = vec![test_case(
            "trivial test",
            vec![assertion(AssertionKind::ToBe, "expect(1).toBe(1)")],
        )];
        let rule = TrivialAssertionRule::new();
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("x")
            .unwrap();
        let issues = rule.analyze(&tests, "", &tree);
        assert!(!issues.is_empty());
        assert!(issues.iter().any(|i| i.rule == Rule::TrivialAssertion));
    }

    #[test]
    fn flags_true_tobe_true() {
        let tests = vec![test_case(
            "always passes",
            vec![assertion(AssertionKind::ToBe, "expect(true).toBe(true)")],
        )];
        let rule = TrivialAssertionRule::new();
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("x")
            .unwrap();
        let issues = rule.analyze(&tests, "", &tree);
        assert!(!issues.is_empty());
    }

    #[test]
    fn no_issue_for_meaningful_assertion() {
        let tests = vec![test_case(
            "real test",
            vec![assertion(AssertionKind::ToBe, "expect(myFunc()).toBe(42)")],
        )];
        let rule = TrivialAssertionRule::new();
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("x")
            .unwrap();
        let issues = rule.analyze(&tests, "", &tree);
        assert!(issues.is_empty());
    }
}
