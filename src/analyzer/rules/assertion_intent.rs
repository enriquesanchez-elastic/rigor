//! Assertion–intent relevance: does the test verify what its name claims?
//!
//! Flags tests where the name implies a specific outcome (e.g. "returns 404",
//! "throws ValidationError", "is empty when no items") but no assertion
//! actually checks that outcome. Such tests are not *meaningful* — they don't
//! test what they say they test.

use super::AnalysisRule;
use crate::{AssertionKind, Issue, Rule, Severity, TestCase};
use tree_sitter::Tree;

/// Rule that detects mismatch between test name (intent) and assertions (what is actually checked).
pub struct AssertionIntentRule;

impl AssertionIntentRule {
    pub fn new() -> Self {
        Self
    }

    /// Name implies a specific return value (e.g. "returns 404", "returns null", "returns []").
    fn name_implies_return_value(name: &str) -> bool {
        let n = name.to_lowercase();
        n.contains("returns ")
            || n.contains("return ")
            || n.contains("should return")
            || n.contains("gives ")
            || n.contains("yields ")
    }

    /// Name implies an error/exception (e.g. "throws", "rejects", "throws error").
    fn name_implies_throws(name: &str) -> bool {
        let n = name.to_lowercase();
        n.contains("throws")
            || n.contains("rejects")
            || n.contains("throw ")
            || n.contains("reject ")
            || n.contains("should throw")
            || n.contains("expect error")
            || n.contains("when invalid")
            || n.contains("when error")
    }

    /// Name implies HTTP status or status code (e.g. "404", "200", "status code").
    fn name_implies_status(name: &str) -> bool {
        let n = name.to_lowercase();
        n.contains("404")
            || n.contains("200")
            || n.contains("201")
            || n.contains("400")
            || n.contains("500")
            || n.contains("status code")
            || n.contains("status ")
            || n.contains("responds with")
    }

    /// Name implies empty / zero length (e.g. "empty", "no items", "zero results").
    fn name_implies_empty(name: &str) -> bool {
        let n = name.to_lowercase();
        n.contains("empty")
            || n.contains("no items")
            || n.contains("no results")
            || n.contains("zero ")
            || n.contains("length 0")
            || n.contains("has no ")
    }

    /// Assertions actually verify a throw/reject.
    fn has_throw_assertion(assertions: &[crate::Assertion]) -> bool {
        assertions.iter().any(|a| {
            matches!(a.kind, AssertionKind::ToThrow)
                || a.raw.contains("rejects")
                || a.raw.contains("toThrow")
        })
    }

    /// Assertions check status code (e.g. status, statusCode, .status).
    fn has_status_assertion(assertions: &[crate::Assertion]) -> bool {
        assertions.iter().any(|a| {
            let r = a.raw.to_lowercase();
            r.contains("status")
                || r.contains("statuscode")
                || r.contains(".status")
                || r.contains("statuscode")
        })
    }

    /// Assertions check for empty (length 0, toHaveLength(0), empty array, etc.).
    fn has_empty_assertion(assertions: &[crate::Assertion]) -> bool {
        assertions.iter().any(|a| {
            let r = a.raw.to_lowercase();
            // Jest/Vitest patterns
            r.contains("tohavelength(0)")
                || r.contains("to have length 0")
                || (r.contains("length") && r.contains("0"))
                || r.contains("toequal([])")
                || r.contains("tostrictequal([])")
                || r.contains("toequal({})")
                || r.contains("tostrictequal({})")
                || r.contains(".length).tobe(0)")
                || r.contains(".length).toequal(0)")
                // jest-extended patterns
                || r.contains("tobeempty()")
                || r.contains("tobearrayofsize(0)")
                // Cypress patterns
                || r.contains("should('have.length', 0)")
                || r.contains("should(\"have.length\", 0)")
                || r.contains("should('be.empty')")
                || r.contains("should(\"be.empty\")")
                // Generic patterns
                || r.contains("empty")
        })
    }

    /// Assertions verify a specific value (toBe, toEqual with a literal or variable).
    fn has_specific_value_assertion(assertions: &[crate::Assertion]) -> bool {
        assertions.iter().any(|a| {
            matches!(
                a.kind,
                AssertionKind::ToBe
                    | AssertionKind::ToEqual
                    | AssertionKind::ToStrictEqual
                    | AssertionKind::ToHaveLength
                    | AssertionKind::ToBeGreaterThan
                    | AssertionKind::ToBeLessThan
            ) || (a.quality == crate::AssertionQuality::Strong
                && !a.raw.to_lowercase().contains("tobedefined"))
        })
    }
}

impl Default for AssertionIntentRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for AssertionIntentRule {
    fn name(&self) -> &'static str {
        "assertion-intent"
    }

    fn analyze(&self, tests: &[TestCase], _source: &str, _tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();

        for test in tests {
            if test.is_skipped || test.assertions.is_empty() {
                continue;
            }

            let name = test.name.trim();

            // Name says "throws" / "rejects" but no toThrow/rejects in assertions
            if Self::name_implies_throws(name) && !Self::has_throw_assertion(&test.assertions) {
                issues.push(Issue {
                    rule: Rule::AssertionIntentMismatch,
                    severity: Severity::Warning,
                    message: format!(
                        "Test '{}' suggests an error is thrown/rejected but no toThrow or rejects assertion found — test may not verify what it claims",
                        test.name
                    ),
                    location: test.location.clone(),
                    suggestion: Some(
                        "Add expect(fn).toThrow() or expect(promise).rejects.toThrow(...) so the test actually verifies the error.".to_string(),
                    ),
                    fix: None,
                });
            }

            // Name suggests status code but no status assertion
            if Self::name_implies_status(name) && !Self::has_status_assertion(&test.assertions) {
                issues.push(Issue {
                    rule: Rule::AssertionIntentMismatch,
                    severity: Severity::Warning,
                    message: format!(
                        "Test '{}' suggests a status/code is checked but no status assertion found — test may not verify what it claims",
                        test.name
                    ),
                    location: test.location.clone(),
                    suggestion: Some(
                        "Add expect(response.status).toBe(404) or expect(response.statusCode).toBe(200) so the test verifies the status.".to_string(),
                    ),
                    fix: None,
                });
            }

            // Name suggests empty but no empty assertion.
            // Skip when "empty" describes the input (e.g. "for empty email") not the expected result: test has toThrow.
            let empty_is_input_not_result = Self::has_throw_assertion(&test.assertions)
                && name.to_lowercase().contains("empty");
            if Self::name_implies_empty(name)
                && !Self::has_empty_assertion(&test.assertions)
                && !empty_is_input_not_result
            {
                issues.push(Issue {
                    rule: Rule::AssertionIntentMismatch,
                    severity: Severity::Warning,
                    message: format!(
                        "Test '{}' suggests empty/zero result but no length or empty assertion found — test may not verify what it claims",
                        test.name
                    ),
                    location: test.location.clone(),
                    suggestion: Some(
                        "Add expect(result).toHaveLength(0) or expect(result).toEqual([]) so the test verifies emptiness.".to_string(),
                    ),
                    fix: None,
                });
            }

            // Name suggests a specific return value but only weak assertions (e.g. toBeDefined)
            if Self::name_implies_return_value(name)
                && !Self::has_specific_value_assertion(&test.assertions)
            {
                let all_weak = test
                    .assertions
                    .iter()
                    .all(|a| a.quality == crate::AssertionQuality::Weak);
                if all_weak {
                    issues.push(Issue {
                        rule: Rule::AssertionIntentMismatch,
                        severity: Severity::Warning,
                        message: format!(
                            "Test '{}' suggests a specific return value but assertions only check presence (e.g. toBeDefined) — test may not verify the actual value",
                            test.name
                        ),
                        location: test.location.clone(),
                        suggestion: Some(
                            "Add expect(result).toBe(expected) or expect(result).toEqual(expected) with the specific value so the test verifies what it claims.".to_string(),
                        ),
                    fix: None,
                    });
                }
            }
        }

        issues
    }

    fn calculate_score(&self, tests: &[TestCase], issues: &[Issue]) -> u8 {
        let mismatch_count = issues
            .iter()
            .filter(|i| i.rule == Rule::AssertionIntentMismatch)
            .count();
        if tests.is_empty() {
            return 25;
        }
        let deduction = (mismatch_count as i32 * 3).min(15);
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
    fn flags_throws_name_without_to_throw() {
        let tests = vec![test_case(
            "throws when input is invalid",
            vec![assertion(
                AssertionKind::ToBeDefined,
                "expect(result).toBeDefined()",
            )],
        )];
        let rule = AssertionIntentRule::new();
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("x")
            .unwrap();
        let issues = rule.analyze(&tests, "", &tree);
        assert!(!issues.is_empty());
        assert!(issues
            .iter()
            .any(|i| i.rule == Rule::AssertionIntentMismatch
                && i.message.contains("error is thrown")));
    }

    #[test]
    fn no_issue_when_throws_and_has_to_throw() {
        let tests = vec![test_case(
            "throws when input is invalid",
            vec![assertion(
                AssertionKind::ToThrow,
                "expect(() => fn()).toThrow()",
            )],
        )];
        let rule = AssertionIntentRule::new();
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("x")
            .unwrap();
        let issues = rule.analyze(&tests, "", &tree);
        assert!(issues.is_empty() || !issues.iter().any(|i| i.message.contains("error is thrown")));
    }

    #[test]
    fn flags_status_name_without_status_assertion() {
        let tests = vec![test_case(
            "returns 404 when not found",
            vec![assertion(
                AssertionKind::ToBeDefined,
                "expect(res).toBeDefined()",
            )],
        )];
        let rule = AssertionIntentRule::new();
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("x")
            .unwrap();
        let issues = rule.analyze(&tests, "", &tree);
        assert!(issues
            .iter()
            .any(|i| i.rule == Rule::AssertionIntentMismatch));
    }
}
