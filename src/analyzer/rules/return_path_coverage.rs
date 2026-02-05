//! Return path coverage analysis rule
//!
//! Flags when source functions have multiple return paths but tests likely
//! only cover a subset (e.g. only positive inputs when function has negative/zero/positive paths).

use super::AnalysisRule;
use crate::parser::SourceFileParser;
use crate::{Issue, Location, Rule, Severity, TestCase};
use std::collections::HashSet;
use tree_sitter::Tree;

/// Find the test file location that best points the user to where to add tests (first test that references the function).
fn test_location_for_function(tests: &[TestCase], fn_name: &str) -> Location {
    let fn_lower = fn_name.to_lowercase();
    for test in tests {
        let name_has = test.name.to_lowercase().contains(&fn_lower);
        let assertions_mention = test
            .assertions
            .iter()
            .any(|a| a.raw.to_lowercase().contains(&fn_lower));
        if name_has || assertions_mention {
            return test.location.clone();
        }
    }
    tests
        .first()
        .map(|t| t.location.clone())
        .unwrap_or_else(|| Location::new(1, 1))
}

/// Rule for analyzing return path coverage
pub struct ReturnPathCoverageRule {
    source_content: Option<String>,
    source_tree: Option<Tree>,
}

impl ReturnPathCoverageRule {
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

    /// Estimate how many return paths are likely covered by tests based on test names and assertion content
    fn estimate_covered_paths(
        &self,
        fn_name: &str,
        tests: &[TestCase],
        test_source: &str,
    ) -> usize {
        let fn_lower = fn_name.to_lowercase();
        let source_lower = test_source.to_lowercase();
        let mut path_hints = HashSet::new();

        if !source_lower.contains(&fn_lower) {
            return 0;
        }

        for test in tests {
            let name_lower = test.name.to_lowercase();
            let relevant = name_lower.contains(&fn_lower)
                || name_lower.contains("process")
                || name_lower.contains("handle")
                || test
                    .assertions
                    .iter()
                    .any(|a| a.raw.to_lowercase().contains(&fn_lower));
            if !relevant {
                continue;
            }
            if name_lower.contains("zero")
                || name_lower.contains(" 0 ")
                || name_lower.contains("(0)")
            {
                path_hints.insert("zero");
            }
            if name_lower.contains("negative")
                || name_lower.contains("invalid")
                || name_lower.contains("error")
            {
                path_hints.insert("negative");
            }
            if name_lower.contains("positive")
                || name_lower.contains("valid")
                || name_lower.contains("success")
            {
                path_hints.insert("positive");
            }
            if name_lower.contains("empty")
                || name_lower.contains("null")
                || name_lower.contains("undefined")
            {
                path_hints.insert("empty");
            }
            if name_lower.contains("boundary") || name_lower.contains("edge") {
                path_hints.insert("boundary");
            }
            // Literal 0 or negative in assertions suggests that path is tested
            for a in &test.assertions {
                let raw = a.raw.to_lowercase();
                if raw.contains("(0)")
                    || raw.contains(", 0)")
                    || raw.contains("=== 0")
                    || raw.contains("=== -0")
                {
                    path_hints.insert("zero");
                }
                if raw.contains("-1") || raw.contains("negative") || raw.contains("< 0") {
                    path_hints.insert("negative");
                }
            }
        }

        if path_hints.is_empty() {
            return 1;
        }
        path_hints.len()
    }

    /// Build a concrete suggestion from return statement condition contexts when available
    fn path_suggestion(func: &crate::parser::FunctionDetails) -> String {
        let contexts: Vec<String> = func
            .return_statements
            .iter()
            .filter_map(|r| r.condition_context.as_ref().cloned())
            .take(5)
            .collect();
        if contexts.is_empty() {
            format!(
                "Add tests for each branch (function has {} return path(s)). Try: zero input, negative/invalid input, and the default case.",
                func.return_paths
            )
        } else {
            format!(
                "Add tests for: {}. Also cover the default/fallback path.",
                contexts.join("; ")
            )
        }
    }
}

impl Default for ReturnPathCoverageRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for ReturnPathCoverageRule {
    fn name(&self) -> &'static str {
        "return-path-coverage"
    }

    fn analyze(&self, tests: &[TestCase], source: &str, _tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();

        if let (Some(ref source_content), Some(ref source_tree)) =
            (&self.source_content, &self.source_tree)
        {
            let parser = SourceFileParser::new(source_content);
            let details = parser.extract_function_details(source_tree);

            for func in details {
                // Only flag functions with multiple return paths
                if func.return_paths < 2 {
                    continue;
                }

                let estimated_covered = self.estimate_covered_paths(&func.name, tests, source);
                let total_paths = func.return_paths;
                let coverage_ratio = if total_paths > 0 {
                    estimated_covered as f64 / total_paths as f64
                } else {
                    1.0
                };

                if coverage_ratio < 0.66 {
                    let percent = (coverage_ratio * 100.0) as u32;
                    let location = test_location_for_function(tests, &func.name);
                    issues.push(Issue {
                        rule: Rule::ReturnPathCoverage,
                        severity: Severity::Warning,
                        message: format!(
                            "Function '{}' has {} return path(s) but tests likely cover only ~{}% ({} of {} paths)",
                            func.name, total_paths, percent, estimated_covered, total_paths
                        ),
                        location,
                        suggestion: Some(Self::path_suggestion(&func)),
                    });
                }
            }
        }

        issues
    }

    fn calculate_score(&self, _tests: &[TestCase], issues: &[Issue]) -> u8 {
        let path_issues = issues
            .iter()
            .filter(|i| i.rule == Rule::ReturnPathCoverage)
            .count();
        let deduction = (path_issues as i32 * 5).min(25);
        (25 - deduction).max(0) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Issue, Location, Severity, TestCase};

    #[test]
    fn negative_no_source_returns_empty() {
        let rule = ReturnPathCoverageRule::new();
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("it('test', () => {});")
            .unwrap();
        let tests: Vec<TestCase> = vec![];
        let test_source = "";
        let issues = rule.analyze(&tests, test_source, &tree);
        assert!(issues.is_empty());
    }

    #[test]
    fn positive_with_source_multiple_paths_under_tested() {
        let source_content = r#"
        function classify(n: number): number {
            if (n < 0) return -1;
            if (n === 0) return 0;
            return 1;
        }
        "#;
        let mut parser = crate::parser::TypeScriptParser::new().unwrap();
        let source_tree = parser.parse(source_content).unwrap();
        let rule = ReturnPathCoverageRule::new().with_source(
            source_content.to_string(),
            source_tree,
        );
        let tests = vec![TestCase {
            name: "positive case".to_string(),
            location: Location::new(2, 1),
            is_async: false,
            is_skipped: false,
            assertions: vec![],
            describe_block: None,
        }];
        let test_source = "expect(classify(5)).toBe(1);";
        let tree = parser.parse(test_source).unwrap();
        let issues = rule.analyze(&tests, test_source, &tree);
        assert!(
            issues.iter().any(|i| i.rule == Rule::ReturnPathCoverage),
            "expected ReturnPathCoverage when multiple return paths and tests cover only one"
        );
    }

    #[test]
    fn score_decreases_with_issues() {
        let rule = ReturnPathCoverageRule::new();
        let tests: Vec<TestCase> = vec![];
        let zero_issues: Vec<Issue> = vec![];
        let one_issue = vec![Issue {
            rule: Rule::ReturnPathCoverage,
            severity: Severity::Warning,
            message: "test".to_string(),
            location: Location::new(1, 1),
            suggestion: None,
        }];
        assert_eq!(rule.calculate_score(&tests, &zero_issues), 25);
        assert_eq!(rule.calculate_score(&tests, &one_issue), 20);
    }
}
