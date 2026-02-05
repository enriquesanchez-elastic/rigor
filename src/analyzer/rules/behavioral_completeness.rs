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

    /// Find which return object keys are asserted: from test source (result.status) and from assertion.raw (e.g. .status, status:)
    fn asserted_keys_in_tests(
        test_source: &str,
        tests: &[TestCase],
        fn_name: &str,
    ) -> HashSet<String> {
        let mut asserted = HashSet::new();
        let source_lower = test_source.to_lowercase();
        let fn_lower = fn_name.to_lowercase();

        if !source_lower.contains(&fn_lower) {
            return asserted;
        }

        // From assertion raw text: .property (e.g. expect(result.status).toBe(...))
        for test in tests {
            for a in &test.assertions {
                let raw = a.raw.to_lowercase();
                let mut i = 0;
                while i < raw.len() {
                    if raw[i..].starts_with('.') {
                        let rest = &raw[i + 1..];
                        let end = rest
                            .find(|c: char| !c.is_alphanumeric() && c != '_')
                            .unwrap_or(rest.len());
                        let prop = rest[..end].to_string();
                        if !prop.is_empty() && prop != "then" && prop != "catch" && prop.len() < 40
                        {
                            asserted.insert(prop);
                        }
                        i += 1 + end;
                    } else {
                        i += 1;
                    }
                }
            }
        }

        // From source: result.property, response.property, etc.
        let result_vars = [
            "result", "response", "res", "data", "output", "value", "ret",
        ];
        for var in result_vars {
            let needle = format!("{}.", var);
            if !source_lower.contains(&needle) {
                continue;
            }
            let mut search_start = 0;
            while let Some(pos) = source_lower[search_start..].find(&needle) {
                let start = search_start + pos + needle.len();
                let rest = &source_lower[start..];
                let end = rest
                    .find(|c: char| !c.is_alphanumeric() && c != '_')
                    .unwrap_or(rest.len());
                let prop = rest[..end].to_string();
                if !prop.is_empty() && prop != "then" && prop != "catch" {
                    asserted.insert(prop);
                }
                search_start = start + end;
                if search_start >= source_lower.len() {
                    break;
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
