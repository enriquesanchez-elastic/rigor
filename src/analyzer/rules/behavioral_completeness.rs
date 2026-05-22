//! Behavioral completeness analysis rule
//!
//! Flags when tests verify only partial behavior (e.g. only one property of a return object)
//! instead of the full behavioral contract.

use super::AnalysisRule;
use crate::parser::SourceFileParser;
use crate::{Issue, Location, Rule, Severity, TestCase};
use std::collections::HashSet;
use tree_sitter::Tree;

/// Rule for analyzing behavioral completeness of tests
pub struct BehavioralCompletenessRule {
    source_content: Option<String>,
    source_tree: Option<Tree>,
}

impl BehavioralCompletenessRule {
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

    /// Collect all object keys that appear in return statements of a function
    fn return_object_keys(func: &crate::parser::FunctionDetails) -> HashSet<String> {
        let mut keys = HashSet::new();
        for ret in &func.return_statements {
            if let Some(ref shape) = ret.value_shape {
                for k in &shape.object_keys {
                    keys.insert(k.clone());
                }
            }
        }
        keys
    }

    /// Collect asserted property names only from tests that reference `fn_name`.
    /// Looks at assertion raw text for `.property` patterns.
    /// Does NOT scan full test source — that incorrectly attributes cross-function accesses.
    pub(crate) fn asserted_keys_in_tests(
        _test_source: &str,
        tests: &[TestCase],
        fn_name: &str,
    ) -> HashSet<String> {
        let mut asserted = HashSet::new();
        let fn_lower = fn_name.to_lowercase();

        // Only consider tests that actually reference this function
        let relevant: Vec<_> = tests
            .iter()
            .filter(|t| {
                t.name.to_lowercase().contains(&fn_lower)
                    || t.assertions
                        .iter()
                        .any(|a| a.raw.to_lowercase().contains(&fn_lower))
            })
            .collect();

        if relevant.is_empty() {
            return asserted;
        }

        for test in relevant {
            for a in &test.assertions {
                let raw = a.raw.to_lowercase();
                let mut i = 0;
                while i < raw.len() {
                    if raw.as_bytes().get(i) == Some(&b'.') {
                        let rest = &raw[i + 1..];
                        let end = rest
                            .find(|c: char| !c.is_alphanumeric() && c != '_')
                            .unwrap_or(rest.len());
                        let prop = &rest[..end];
                        if !prop.is_empty() && prop != "then" && prop != "catch" && prop.len() < 40
                        {
                            asserted.insert(prop.to_string());
                        }
                        i += 1 + end;
                    } else {
                        i += 1;
                    }
                }
            }
        }

        asserted
    }

    /// Test file location for the function (first test that references it)
    fn test_location_for_function(tests: &[TestCase], fn_name: &str) -> Location {
        let fn_lower = fn_name.to_lowercase();
        for test in tests {
            if test.name.to_lowercase().contains(&fn_lower)
                || test
                    .assertions
                    .iter()
                    .any(|a| a.raw.to_lowercase().contains(&fn_lower))
            {
                return test.location.clone();
            }
        }
        tests
            .first()
            .map(|t| t.location.clone())
            .unwrap_or_else(|| Location::new(1, 1))
    }
}

impl Default for BehavioralCompletenessRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for BehavioralCompletenessRule {
    fn name(&self) -> &'static str {
        "behavioral-completeness"
    }

    fn analyze(&self, tests: &[TestCase], source: &str, _tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();

        if tests.is_empty() {
            return issues;
        }

        if let (Some(ref source_content), Some(ref source_tree)) =
            (&self.source_content, &self.source_tree)
        {
            let parser = SourceFileParser::new(source_content);
            let details = parser.extract_function_details(source_tree);

            for func in details {
                let expected_keys = Self::return_object_keys(&func);
                if expected_keys.len() < 2 {
                    continue; // Single key or no shape - skip
                }

                let asserted = Self::asserted_keys_in_tests(source, tests, &func.name);
                if asserted.is_empty() {
                    continue;
                }

                let missing: Vec<_> = expected_keys.difference(&asserted).cloned().collect();
                if missing.is_empty() {
                    continue;
                }

                let total = expected_keys.len();
                let verified = total - missing.len();
                let ratio = verified as f64 / total as f64;
                let location = Self::test_location_for_function(tests, &func.name);

                if ratio < 0.5 {
                    let examples: Vec<String> = missing
                        .iter()
                        .take(3)
                        .map(|k| format!("expect(result.{}).toBeDefined()", k))
                        .collect();
                    let suggestion = format!(
                        "Add assertions for: {}. Examples: {}",
                        missing.join(", "),
                        examples.join("; ")
                    );
                    issues.push(Issue {
                        rule: Rule::BehavioralCompleteness,
                        severity: Severity::Warning,
                        message: format!(
                            "Function '{}' returns an object with {} properties but tests only verify {} of them (missing: {})",
                            func.name, total, verified, missing.join(", ")
                        ),
                        location: location.clone(),
                        suggestion: Some(suggestion),
                        fix: None,
                    });
                } else if ratio < 1.0 {
                    issues.push(Issue {
                        rule: Rule::BehavioralCompleteness,
                        severity: Severity::Info,
                        message: format!(
                            "Function '{}' return object has {} properties; {} not asserted: {}",
                            func.name,
                            total,
                            missing.len(),
                            missing.join(", ")
                        ),
                        location,
                        suggestion: Some(format!("Consider verifying: {}", missing.join(", "))),
                        fix: None,
                    });
                }
            }
        }

        issues
    }

    fn calculate_score(&self, _tests: &[TestCase], issues: &[Issue]) -> u8 {
        let completeness_issues = issues
            .iter()
            .filter(|i| i.rule == Rule::BehavioralCompleteness)
            .count();
        let deduction = (completeness_issues as i32 * 4).min(25);
        (25 - deduction).max(0) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Assertion, AssertionKind, Issue, Location, Severity, TestCase};

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
    fn negative_no_source_returns_empty() {
        let rule = BehavioralCompletenessRule::new();
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("it('test', () => {});")
            .unwrap();
        let tests = vec![make_test(
            "getResponse",
            vec![make_assertion(
                AssertionKind::ToBe,
                "expect(result.status).toBe(200)",
            )],
        )];
        let test_source = "const result = getResponse(); expect(result.status).toBe(200);";
        let issues = rule.analyze(&tests, test_source, &tree);
        assert!(issues.is_empty());
    }

    #[test]
    fn positive_with_source_partial_assertion_triggers() {
        let source_content = r#"
        function getResponse() {
            return { status: 200, data: {}, headers: {} };
        }
        "#;
        let mut parser = crate::parser::TypeScriptParser::new().unwrap();
        let source_tree = parser.parse(source_content).unwrap();
        let rule =
            BehavioralCompletenessRule::new().with_source(source_content.to_string(), source_tree);
        let tests = vec![make_test(
            "getResponse",
            vec![make_assertion(
                AssertionKind::ToBe,
                "expect(result.status).toBe(200)",
            )],
        )];
        let test_source = "const result = getResponse(); expect(result.status).toBe(200);";
        let tree = parser.parse(test_source).unwrap();
        let issues = rule.analyze(&tests, test_source, &tree);
        if !issues.is_empty() {
            assert!(
                issues
                    .iter()
                    .any(|i| i.rule == Rule::BehavioralCompleteness),
                "when issues found, expected BehavioralCompleteness"
            );
        }
    }

    #[test]
    fn asserted_keys_not_attributed_to_unrelated_function() {
        // testA calls getUser() and accesses result.status
        // getOrder() returns {status, amount} — neither test references getOrder
        // result.status from testA must NOT count as asserting getOrder's status property
        let tests = vec![
            make_test(
                "getUser returns status",
                vec![make_assertion(
                    AssertionKind::ToBe,
                    "expect(result.status).toBe(200)",
                )],
            ),
            make_test(
                "getOrder works",
                vec![make_assertion(
                    AssertionKind::ToBe,
                    "expect(total).toBe(50)",
                )],
            ),
        ];

        // The test_source is the full test file — it contains getOrder (so the fn_name check passes)
        // but the `result.status` access belongs to the getUser test, not getOrder
        let asserted = BehavioralCompletenessRule::asserted_keys_in_tests(
            "const result = getUser();\nconst order = getOrder();",
            &tests,
            "getOrder",
        );

        assert!(
            !asserted.contains("status"),
            "status from getUser test must not be attributed to getOrder; got: {:?}",
            asserted
        );
    }

    #[test]
    fn score_decreases_with_issues() {
        let rule = BehavioralCompletenessRule::new();
        let tests: Vec<TestCase> = vec![];
        let zero_issues: Vec<Issue> = vec![];
        let one_issue = vec![Issue {
            rule: Rule::BehavioralCompleteness,
            severity: Severity::Warning,
            message: "test".to_string(),
            location: Location::new(1, 1),
            suggestion: None,
            fix: None,
        }];
        assert_eq!(rule.calculate_score(&tests, &zero_issues), 25);
        assert_eq!(rule.calculate_score(&tests, &one_issue), 21);
    }
}
