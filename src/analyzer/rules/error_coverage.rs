//! Error coverage analysis rule

use super::AnalysisRule;
use crate::parser::SourceFileParser;
use crate::{AssertionKind, Issue, Location, Rule, Severity, TestCase};
use tree_sitter::Tree;

/// Rule for analyzing error handling coverage
pub struct ErrorCoverageRule {
    source_content: Option<String>,
    source_tree: Option<Tree>,
}

impl ErrorCoverageRule {
    pub fn new() -> Self {
        Self {
            source_content: None,
            source_tree: None,
        }
    }

    /// Set the corresponding source file content for analysis
    pub fn with_source(mut self, content: String, tree: Tree) -> Self {
        self.source_content = Some(content);
        self.source_tree = Some(tree);
        self
    }

    fn has_error_test(tests: &[TestCase], fn_name: &str) -> bool {
        for test in tests {
            // Check if test name mentions the function and error handling
            let name_lower = test.name.to_lowercase();
            let fn_name_lower = fn_name.to_lowercase();

            let mentions_function =
                name_lower.contains(&fn_name_lower) || name_lower.contains("error");

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

        // Also check if tests that should test errors actually use toThrow
        for test in tests {
            let name_lower = test.name.to_lowercase();
            let should_test_error = name_lower.contains("throw")
                || name_lower.contains("error")
                || name_lower.contains("fail")
                || name_lower.contains("invalid")
                || name_lower.contains("reject");

            if should_test_error {
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
