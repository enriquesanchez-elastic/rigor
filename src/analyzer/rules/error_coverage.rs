//! Error coverage analysis rule

use super::AnalysisRule;
use crate::parser::SourceFileParser;
use crate::{AssertionKind, Issue, Location, Rule, Severity, TestCase, TestType};
use tree_sitter::Tree;

/// Rule for analyzing error handling coverage
pub struct ErrorCoverageRule {
    source_content: Option<String>,
    source_tree: Option<Tree>,
    /// When E2e, skip per-test "appears to test error handling" heuristic (e2e tests check error display, not exceptions).
    test_type: TestType,
}

impl ErrorCoverageRule {
    pub fn new() -> Self {
        Self {
            source_content: None,
            source_tree: None,
            test_type: TestType::Unit,
        }
    }

    /// Set the corresponding source file content for analysis
    pub fn with_source(mut self, content: String, tree: Tree) -> Self {
        self.source_content = Some(content);
        self.source_tree = Some(tree);
        self
    }

    /// Set test type (e.g. E2e) to adjust heuristics
    pub fn with_test_type(mut self, test_type: TestType) -> Self {
        self.test_type = test_type;
        self
    }

    fn has_error_test(tests: &[TestCase], fn_name: &str) -> bool {
        for test in tests {
            // Check if test name mentions the function and error handling
            let name_lower = test.name.to_lowercase();
            let fn_name_lower = fn_name.to_lowercase();

            // The function name must actually appear in the test name.
            // Previously `name_lower.contains("error")` was here too, which caused
            // any test with "error" in its name to match ANY function — a false positive.
            let mentions_function = name_lower.contains(&fn_name_lower);

            let mentions_error = name_lower.contains("throw")
                || name_lower.contains("error")
                || name_lower.contains("fail")
                || name_lower.contains("invalid")
                || name_lower.contains("reject");

            if mentions_function && mentions_error {
                return true;
            }

            // Check if test has toThrow assertions
            let has_throw_assertion = test.assertions.iter().any(|a| {
                matches!(a.kind, AssertionKind::ToThrow)
                    || matches!(&a.kind, AssertionKind::Negated(inner) if matches!(**inner, AssertionKind::ToThrow))
            });

            if has_throw_assertion && name_lower.contains(&fn_name_lower) {
                return true;
            }
        }

        false
    }
}

impl Default for ErrorCoverageRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for ErrorCoverageRule {
    fn name(&self) -> &'static str {
        "error-coverage"
    }

    fn analyze(&self, tests: &[TestCase], _source: &str, _tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();

        // If we have source file, analyze throwable functions
        if let (Some(source_content), Some(source_tree)) = (&self.source_content, &self.source_tree)
        {
            let parser = SourceFileParser::new(source_content);
            let throwables = parser.extract_throwable_functions(source_tree);

            for throwable in throwables {
                if !Self::has_error_test(tests, &throwable.name) {
                    let error_types = if throwable.error_types.is_empty() {
                        "errors".to_string()
                    } else {
                        throwable.error_types.join(", ")
                    };

                    issues.push(Issue {
                        rule: Rule::MissingErrorTest,
                        severity: Severity::Warning,
                        message: format!(
                            "Function '{}' can throw {} but has no error test",
                            throwable.name, error_types
                        ),
                        location: Location::new(1, 1), // Test file location
                        suggestion: Some(format!(
                            "Add: it('throws for invalid input', () => {{ expect(() => {}(invalid)).toThrow(ErrorType); }})",
                            throwable.name
                        )),
                    });
                }
            }
        }

        // Skip per-test "appears to test error handling" heuristic for E2e (they check error display, not exceptions).
        if self.test_type != TestType::E2e {
            for test in tests {
                let name_lower = test.name.to_lowercase();
                let should_test_error = name_lower.contains("throw")
                    || name_lower.contains("error")
                    || name_lower.contains("fail")
                    || name_lower.contains("invalid")
                    || name_lower.contains("reject");

                if should_test_error {
                    // "invalid X is falsy" tests boolean return, not exceptions — don't flag
                    let only_invalid = name_lower.contains("invalid")
                        && !name_lower.contains("throw")
                        && !name_lower.contains("reject")
                        && !name_lower.contains("error");
                    let has_boolean_assertion = test.assertions.iter().any(|a| {
                        let r = a.raw.to_lowercase();
                        r.contains("tobefalsy")
                            || r.contains("tobe(false)")
                            || r.contains("tobe(true)")
                    });
                    if only_invalid && has_boolean_assertion {
                        continue;
                    }

                    let has_error_assertion = test.assertions.iter().any(|a| {
                        matches!(a.kind, AssertionKind::ToThrow)
                            || a.raw.contains("rejects")
                            || a.raw.contains("catch")
                    });

                    if !has_error_assertion && !test.assertions.is_empty() {
                        issues.push(Issue {
                            rule: Rule::MissingErrorTest,
                            severity: Severity::Info,
                            message: format!(
                                "Test '{}' appears to test error handling but has no toThrow/rejects assertion",
                                test.name
                            ),
                            location: test.location.clone(),
                            suggestion: Some(
                                "Sync: expect(() => fn(bad)).toThrow(ErrorType); Async: await expect(fn()).rejects.toThrow('message')".to_string()
                            ),
                        });
                    }
                }
            }
        }

        issues
    }

    fn calculate_score(&self, tests: &[TestCase], issues: &[Issue]) -> u8 {
        if tests.is_empty() {
            return 0;
        }

        let mut score: i32 = 25;

        // Count missing error tests
        let missing_error_tests = issues
            .iter()
            .filter(|i| i.rule == Rule::MissingErrorTest && i.severity == Severity::Warning)
            .count();

        // Deduct for missing error tests (-4 each, max -20)
        score -= (missing_error_tests as i32 * 4).min(20);

        // Deduct for error tests without proper assertions (-2 each, max -10)
        let weak_error_tests = issues
            .iter()
            .filter(|i| i.rule == Rule::MissingErrorTest && i.severity == Severity::Info)
            .count();
        score -= (weak_error_tests as i32 * 2).min(10);

        // Bonus for tests that properly test errors
        let proper_error_tests = tests
            .iter()
            .filter(|t| {
                t.assertions
                    .iter()
                    .any(|a| matches!(a.kind, AssertionKind::ToThrow))
            })
            .count();

        if proper_error_tests > 0 {
            score += (proper_error_tests as i32).min(5);
        }

        score.clamp(0, 25) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Assertion, Location};

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

    fn make_throw_assertion() -> Assertion {
        Assertion {
            kind: AssertionKind::ToThrow,
            quality: AssertionKind::ToThrow.quality(),
            location: Location::new(1, 1),
            raw: "expect(() => fn()).toThrow()".to_string(),
        }
    }

    #[test]
    fn test_error_test_detection() {
        // Test must reference the function name (e.g. in assertion), not just the word "error"
        let mut throw_assertion = make_throw_assertion();
        throw_assertion.raw = "expect(() => parseInput(invalid)).toThrow()".to_string();
        let tests = vec![make_test(
            "parseInput throws on invalid input",
            vec![throw_assertion],
        )];

        assert!(ErrorCoverageRule::has_error_test(&tests, "parseInput"));
        assert!(!ErrorCoverageRule::has_error_test(&tests, "otherFunction"));
    }

    #[test]
    fn test_error_keyword_in_name_does_not_match_unrelated_function() {
        // Regression: a test named "should handle error for X" must NOT match
        // a completely unrelated function just because "error" appears in the name.
        let tests = vec![make_test(
            "should handle error for submitForm",
            vec![make_throw_assertion()],
        )];

        // Matches submitForm because the name contains "submitform" + "error"
        assert!(ErrorCoverageRule::has_error_test(&tests, "submitForm"));
        // Must NOT match an unrelated function — "error" in the name is not enough
        assert!(
            !ErrorCoverageRule::has_error_test(&tests, "calculateTax"),
            "test with 'error' in name should not match unrelated function 'calculateTax'"
        );
        assert!(
            !ErrorCoverageRule::has_error_test(&tests, "validateAge"),
            "test with 'error' in name should not match unrelated function 'validateAge'"
        );
    }

    #[test]
    fn test_missing_error_assertion() {
        let tests = vec![make_test(
            "should throw error",
            vec![Assertion {
                kind: AssertionKind::ToBe,
                quality: AssertionKind::ToBe.quality(),
                location: Location::new(1, 1),
                raw: "expect(x).toBe(1)".to_string(),
            }],
        )];

        let rule = ErrorCoverageRule::new();
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("test")
            .unwrap();
        let issues = rule.analyze(&tests, "", &tree);

        assert!(!issues.is_empty());
        assert!(issues.iter().any(|i| i.message.contains("error handling")));
    }
}
