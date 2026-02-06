//! Test isolation analysis rule

use super::AnalysisRule;
use crate::{Issue, Location, Rule, Severity, TestCase};
use tree_sitter::{Node, Tree};

/// Rule for analyzing test isolation
pub struct TestIsolationRule;

impl TestIsolationRule {
    pub fn new() -> Self {
        Self
    }

    /// Check for shared state indicators in source code
    fn find_shared_state(source: &str, tree: &Tree) -> Vec<SharedStateIssue> {
        let mut issues = Vec::new();
        Self::visit_for_shared_state(tree.root_node(), source, &mut issues);
        issues
    }

    fn visit_for_shared_state(node: Node, source: &str, issues: &mut Vec<SharedStateIssue>) {
        let node_text = node.utf8_text(source.as_bytes()).unwrap_or("");

        // Check for module-level let/var declarations (potential shared state)
        if node.kind() == "lexical_declaration" || node.kind() == "variable_declaration" {
            // Check if this is at module level (not inside a function)
            if Self::is_at_module_level(node) {
                // Check if it's let (mutable)
                if node_text.starts_with("let ") {
                    let location = Location::new(
                        node.start_position().row + 1,
                        node.start_position().column + 1,
                    );
                    issues.push(SharedStateIssue {
                        kind: SharedStateKind::MutableModuleVariable,
                        location,
                        description: "Mutable module-level variable".to_string(),
                    });
                }
            }
        }

        // Check for beforeEach/afterEach hooks (good pattern)
        if node.kind() == "call_expression" {
            if let Some(function) = node.child_by_field_name("function") {
                let fn_name = function.utf8_text(source.as_bytes()).unwrap_or("");
                if fn_name == "beforeEach" || fn_name == "afterEach" {
                    let location = Location::new(
                        node.start_position().row + 1,
                        node.start_position().column + 1,
                    );
                    issues.push(SharedStateIssue {
                        kind: SharedStateKind::SetupTeardownHook,
                        location,
                        description: format!("{} hook found", fn_name),
                    });
                }
            }
        }

        // Recurse
        for child in node.named_children(&mut node.walk()) {
            Self::visit_for_shared_state(child, source, issues);
        }
    }

    fn is_at_module_level(node: Node) -> bool {
        let mut current = node.parent();
        while let Some(parent) = current {
            match parent.kind() {
                "program" => return true,
                "function_declaration"
                | "function_expression"
                | "arrow_function"
                | "method_definition" => return false,
                _ => current = parent.parent(),
            }
        }
        true
    }

    fn check_test_dependencies(tests: &[TestCase]) -> Vec<(usize, usize)> {
        let mut dependencies = Vec::new();

        // Look for tests that seem to depend on each other by name pattern
        for (i, test_a) in tests.iter().enumerate() {
            for (j, test_b) in tests.iter().enumerate() {
                if i >= j {
                    continue;
                }

                // Check for sequential naming patterns
                let name_a = test_a.name.to_lowercase();
                let name_b = test_b.name.to_lowercase();

                // "step 1", "step 2" patterns
                if (name_a.contains("step 1") && name_b.contains("step 2"))
                    || (name_a.contains("first") && name_b.contains("then"))
                    || (name_a.contains("creates") && name_b.contains("uses created"))
                {
                    dependencies.push((i, j));
                }
            }
        }

        dependencies
    }
}

impl Default for TestIsolationRule {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
struct SharedStateIssue {
    kind: SharedStateKind,
    location: Location,
    description: String,
}

#[derive(Debug, PartialEq)]
enum SharedStateKind {
    MutableModuleVariable,
    SetupTeardownHook,
}

impl AnalysisRule for TestIsolationRule {
    fn name(&self) -> &'static str {
        "test-isolation"
    }

    fn analyze(&self, tests: &[TestCase], source: &str, tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();

        // Find shared state
        let shared_state = Self::find_shared_state(source, tree);

        // Check for mutable module variables without setup hooks
        let has_setup_hooks = shared_state
            .iter()
            .any(|s| s.kind == SharedStateKind::SetupTeardownHook);

        let mutable_vars: Vec<_> = shared_state
            .iter()
            .filter(|s| s.kind == SharedStateKind::MutableModuleVariable)
            .collect();

        if !mutable_vars.is_empty() && !has_setup_hooks {
            for var in mutable_vars {
                issues.push(Issue {
                    rule: Rule::SharedState,
                    severity: Severity::Warning,
                    message: format!(
                        "Mutable module-level variable without beforeEach reset: {}",
                        var.description
                    ),
                    location: var.location.clone(),
                    suggestion: Some(
                        "Add beforeEach hook to reset shared state, or use const instead"
                            .to_string(),
                    ),
                    fix: None,
                });
            }
        }

        // Check for test dependencies
        let dependencies = Self::check_test_dependencies(tests);
        for (i, j) in dependencies {
            issues.push(Issue {
                rule: Rule::SharedState,
                severity: Severity::Warning,
                message: format!(
                    "Tests '{}' and '{}' may have implicit dependencies",
                    tests[i].name, tests[j].name
                ),
                location: tests[i].location.clone(),
                suggestion: Some("Ensure each test can run independently in any order".to_string()),
                fix: None,
            });
        }

        // Check for duplicate test names
        let mut seen_names: std::collections::HashMap<&str, usize> =
            std::collections::HashMap::new();
        for (i, test) in tests.iter().enumerate() {
            if let Some(&prev_idx) = seen_names.get(test.name.as_str()) {
                issues.push(Issue {
                    rule: Rule::DuplicateTest,
                    severity: Severity::Error,
                    message: format!("Duplicate test name: '{}'", test.name),
                    location: test.location.clone(),
                    suggestion: Some(format!(
                        "Rename one of the tests (lines {} and {})",
                        tests[prev_idx].location.line, test.location.line
                    )),
                    fix: None,
                });
            } else {
                seen_names.insert(&test.name, i);
            }
        }

        issues
    }

    fn calculate_score(&self, tests: &[TestCase], issues: &[Issue]) -> u8 {
        if tests.is_empty() {
            return 0;
        }

        let mut score: i32 = 25;

        // Count shared state issues
        let shared_state_issues = issues
            .iter()
            .filter(|i| i.rule == Rule::SharedState)
            .count();

        // Deduct for shared state issues (-4 each, max -20)
        score -= (shared_state_issues as i32 * 4).min(20);

        // Deduct heavily for duplicate tests (-6 each, max -24)
        let duplicates = issues
            .iter()
            .filter(|i| i.rule == Rule::DuplicateTest)
            .count();
        score -= (duplicates as i32 * 6).min(24);

        score.clamp(0, 25) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::TypeScriptParser;
    use crate::{Location, TestCase};

    #[test]
    fn test_detect_shared_state() {
        let source = r#"
            let sharedData = [];

            describe('tests', () => {
                it('test 1', () => {
                    sharedData.push(1);
                    expect(sharedData).toHaveLength(1);
                });
            });
        "#;

        let mut parser = TypeScriptParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let shared_state = TestIsolationRule::find_shared_state(source, &tree);

        assert!(shared_state
            .iter()
            .any(|s| s.kind == SharedStateKind::MutableModuleVariable));
    }

    #[test]
    fn test_detect_setup_hooks() {
        let source = r#"
            let sharedData = [];

            describe('tests', () => {
                beforeEach(() => {
                    sharedData = [];
                });

                it('test 1', () => {
                    expect(sharedData).toHaveLength(0);
                });
            });
        "#;

        let mut parser = TypeScriptParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let shared_state = TestIsolationRule::find_shared_state(source, &tree);

        assert!(shared_state
            .iter()
            .any(|s| s.kind == SharedStateKind::SetupTeardownHook));
    }

    #[test]
    fn test_analyze_returns_shared_state_issue() {
        let source = r#"
            let shared = 0;
            describe('suite', () => {
                it('uses shared', () => { expect(shared).toBe(0); });
            });
        "#;
        let mut parser = TypeScriptParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let tests = vec![TestCase {
            name: "uses shared".to_string(),
            location: Location::new(3, 1),
            is_async: false,
            is_skipped: false,
            assertions: vec![],
            describe_block: Some("suite".to_string()),
        }];
        let rule = TestIsolationRule::new();
        let issues = rule.analyze(&tests, source, &tree);
        assert!(
            issues.iter().any(|i| i.rule == Rule::SharedState),
            "analyze() should report SharedState for module-level let without beforeEach"
        );
    }
}
