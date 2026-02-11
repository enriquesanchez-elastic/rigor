//! Redundant test: test duplicates another test's coverage.
//!
//! Heuristic: tests are grouped by a normalized assertion signature that includes
//! both the assertion kind AND the subject being tested (the expression inside
//! `expect(...)`). This prevents false positives where two tests use the same
//! assertion kind (e.g. `.toBe()`) but test completely different functions.
//!
//! A group must have 3+ tests to trigger a flag, since pairs of tests with the
//! same pattern are common in boundary and error-variant testing.

use super::AnalysisRule;
use crate::{Issue, Rule, Severity, TestCase};
use std::collections::HashMap;
use tree_sitter::Tree;

/// Minimum number of tests with the same signature before flagging.
/// Pairs (2) are common in boundary/error testing; require 3+ to reduce noise.
const MIN_GROUP_SIZE: usize = 3;

/// Rule for detecting redundant tests
pub struct RedundantTestRule;

impl RedundantTestRule {
    pub fn new() -> Self {
        Self
    }

    /// Build a signature that includes the assertion kind AND the subject function/expression.
    ///
    /// Previous approach used only assertion kind (e.g. "ToBe"), which grouped
    /// `expect(validateAge(18)).toBe(true)` with `expect(clamp(5,0,10)).toBe(5)`.
    /// Now includes the subject so they produce different signatures.
    fn assertion_signature(assertions: &[crate::Assertion]) -> String {
        let mut parts: Vec<String> = assertions
            .iter()
            .map(|a| {
                let subject = Self::extract_subject(&a.raw);
                format!("{}:{:?}", subject, a.kind)
            })
            .collect();
        parts.sort();
        parts.join("|")
    }

    /// Extract the subject expression from assertion raw text.
    ///
    /// Given `expect(validateAge(18)).toBe(true)`, extracts the first identifier
    /// from the expect argument → `"validateAge"`.
    /// Given `expect(1).toBe(1)`, returns `"_literal_"`.
    /// Given `expect(() => authenticate('a', 'b')).toThrow(...)`, extracts `"authenticate"`.
    fn extract_subject(raw: &str) -> String {
        // Find the content inside expect(...)
        let inner = if let Some(start) = raw.find("expect(") {
            let rest = &raw[start + 7..];
            let mut depth = 1;
            let mut end = rest.len();
            for (i, ch) in rest.char_indices() {
                match ch {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            end = i;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            &rest[..end]
        } else {
            raw
        };

        // Skip arrow function prefix: "() => "
        let inner = inner
            .trim()
            .trim_start_matches("() =>")
            .trim_start_matches("async () =>")
            .trim();

        // Extract first identifier (alphanumeric + underscore + dots for property access)
        let start = inner.find(|c: char| c.is_ascii_alphabetic() || c == '_');
        if let Some(start) = start {
            let ident = &inner[start..];
            let end = ident
                .find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.')
                .unwrap_or(ident.len());
            let identifier = &ident[..end];
            if !identifier.is_empty() {
                return identifier.to_string();
            }
        }

        "_literal_".to_string()
    }
}

impl Default for RedundantTestRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for RedundantTestRule {
    fn name(&self) -> &'static str {
        "redundant-test"
    }

    fn analyze(&self, tests: &[TestCase], _source: &str, _tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();
        let mut sig_to_tests: HashMap<String, Vec<(usize, &TestCase)>> = HashMap::new();

        for (i, test) in tests.iter().enumerate() {
            if test.is_skipped {
                continue;
            }
            let sig = Self::assertion_signature(&test.assertions);
            if sig.is_empty() {
                continue;
            }
            sig_to_tests.entry(sig).or_default().push((i, test));
        }

        for (_sig, group) in sig_to_tests {
            if group.len() < MIN_GROUP_SIZE {
                continue;
            }
            // Flag all but the first in the group
            for (_, test) in &group[1..] {
                issues.push(Issue {
                    rule: Rule::RedundantTest,
                    severity: Severity::Info,
                    message: format!(
                        "Test '{}' may duplicate another test (same assertion pattern)",
                        test.name
                    ),
                    location: test.location.clone(),
                    suggestion: Some(
                        "Consider merging with the similar test or testing a different scenario"
                            .to_string(),
                    ),
                    fix: None,
                });
            }
        }

        issues
    }

    fn calculate_score(&self, _tests: &[TestCase], issues: &[Issue]) -> u8 {
        let n = issues
            .iter()
            .filter(|i| i.rule == Rule::RedundantTest)
            .count();
        (25i32 - (n as i32 * 2).min(10)).max(0) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Assertion, AssertionKind, AssertionQuality, Location};

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

    fn make_assertion(raw: &str, kind: AssertionKind) -> Assertion {
        Assertion {
            kind: kind.clone(),
            quality: AssertionQuality::Strong,
            location: Location::new(1, 1),
            raw: raw.to_string(),
        }
    }

    #[test]
    fn extract_subject_function_call() {
        assert_eq!(
            RedundantTestRule::extract_subject("expect(validateAge(18)).toBe(true)"),
            "validateAge"
        );
    }

    #[test]
    fn extract_subject_different_functions() {
        assert_eq!(
            RedundantTestRule::extract_subject("expect(clamp(5, 0, 10)).toBe(5)"),
            "clamp"
        );
    }

    #[test]
    fn extract_subject_arrow_function() {
        assert_eq!(
            RedundantTestRule::extract_subject(
                "expect(() => authenticate('a', 'b')).toThrow(AuthError)"
            ),
            "authenticate"
        );
    }

    #[test]
    fn extract_subject_literal() {
        assert_eq!(
            RedundantTestRule::extract_subject("expect(1).toBe(1)"),
            "_literal_"
        );
    }

    #[test]
    fn extract_subject_property_access() {
        assert_eq!(
            RedundantTestRule::extract_subject("expect(result.user).toBeDefined()"),
            "result.user"
        );
    }

    #[test]
    fn different_functions_not_flagged() {
        let tests = vec![
            make_test(
                "validateAge returns true",
                vec![make_assertion(
                    "expect(validateAge(18)).toBe(true)",
                    AssertionKind::ToBe,
                )],
            ),
            make_test(
                "clamp returns 5",
                vec![make_assertion(
                    "expect(clamp(5, 0, 10)).toBe(5)",
                    AssertionKind::ToBe,
                )],
            ),
        ];
        let rule = RedundantTestRule::new();
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("x")
            .unwrap();
        let issues = rule.analyze(&tests, "", &tree);
        assert!(
            issues.is_empty(),
            "tests calling different functions should not be flagged as redundant"
        );
    }

    #[test]
    fn pair_not_flagged_threshold() {
        // Two tests with the same pattern should NOT be flagged (threshold is 3)
        let tests = vec![
            make_test(
                "age 18",
                vec![make_assertion(
                    "expect(validateAge(18)).toBe(true)",
                    AssertionKind::ToBe,
                )],
            ),
            make_test(
                "age 19",
                vec![make_assertion(
                    "expect(validateAge(19)).toBe(true)",
                    AssertionKind::ToBe,
                )],
            ),
        ];
        let rule = RedundantTestRule::new();
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("x")
            .unwrap();
        let issues = rule.analyze(&tests, "", &tree);
        assert!(
            issues.is_empty(),
            "pair of tests should not trigger redundant-test (threshold is {})",
            MIN_GROUP_SIZE
        );
    }

    #[test]
    fn three_same_pattern_flagged() {
        let tests = vec![
            make_test(
                "test 1",
                vec![make_assertion(
                    "expect(authenticate('a', 'b')).toBeDefined()",
                    AssertionKind::ToBeDefined,
                )],
            ),
            make_test(
                "test 2",
                vec![make_assertion(
                    "expect(authenticate('c', 'd')).toBeDefined()",
                    AssertionKind::ToBeDefined,
                )],
            ),
            make_test(
                "test 3",
                vec![make_assertion(
                    "expect(authenticate('e', 'f')).toBeDefined()",
                    AssertionKind::ToBeDefined,
                )],
            ),
        ];
        let rule = RedundantTestRule::new();
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("x")
            .unwrap();
        let issues = rule.analyze(&tests, "", &tree);
        assert_eq!(
            issues.len(),
            2,
            "3 tests with same pattern should flag 2 (all but first)"
        );
    }
}
