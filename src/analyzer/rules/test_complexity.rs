//! Test complexity: flags tests that are too complex (high cyclomatic complexity or too many assertions).

use super::AnalysisRule;
use crate::parser::{count_branches_with_source, find_assertions_in_body, node_line_count};
use crate::{Issue, Location, Rule, Severity, TestCase};
use tree_sitter::{Node, Tree};

const MAX_ASSERTIONS: usize = 15;
const MAX_LINES: usize = 50;
const MAX_COMPLEXITY: usize = 10;

/// Rule for detecting overly complex tests
pub struct TestComplexityRule;

impl TestComplexityRule {
    pub fn new() -> Self {
        Self
    }

    fn node_text(node: Node, source: &[u8]) -> String {
        node.utf8_text(source).unwrap_or_default().to_string()
    }

    fn is_test_call(node: Node, source: &[u8]) -> bool {
        if node.kind() != "call_expression" {
            return false;
        }
        let func = match node.child_by_field_name("function") {
            Some(f) => f,
            None => return false,
        };
        let name = Self::node_text(func, source);
        if name == "it" || name == "test" {
            return true;
        }
        if func.kind() == "member_expression" {
            let obj = func
                .child_by_field_name("object")
                .map(|o| Self::node_text(o, source));
            let prop = func
                .child_by_field_name("property")
                .map(|p| Self::node_text(p, source));
            if matches!(obj.as_deref(), Some("it") | Some("test"))
                && matches!(prop.as_deref(), Some("skip") | Some("only") | Some("todo"))
            {
                return true;
            }
        }
        false
    }

    fn visit_tests(node: Node, source: &str, _tree: &Tree, issues: &mut Vec<Issue>) {
        if Self::is_test_call(node, source.as_bytes()) {
            if let Some(args) = node.child_by_field_name("arguments") {
                let mut cursor = args.walk();
                let children: Vec<Node> = args.named_children(&mut cursor).collect();
                if children.len() >= 2 {
                    let name_node = children[0];
                    let body = children[1];
                    let name = Self::node_text(name_node, source.as_bytes());
                    let name = name.trim_matches(|c| c == '"' || c == '\'');

                    let assertion_count = find_assertions_in_body(body, source.as_bytes()).len();
                    let lines = node_line_count(body);
                    let complexity = count_branches_with_source(body, source);

                    let (over_assertions, over_lines, over_complexity) = (
                        assertion_count > MAX_ASSERTIONS,
                        lines > MAX_LINES,
                        complexity > MAX_COMPLEXITY,
                    );
                    if over_assertions || over_lines || over_complexity {
                        let reasons: Vec<String> = [
                            over_assertions.then(|| {
                                format!("{} assertions (max {})", assertion_count, MAX_ASSERTIONS)
                            }),
                            over_lines.then(|| format!("{} lines (max {})", lines, MAX_LINES)),
                            over_complexity.then(|| {
                                format!("complexity {} (max {})", complexity, MAX_COMPLEXITY)
                            }),
                        ]
                        .into_iter()
                        .flatten()
                        .collect();
                        let message =
                            format!("Test '{}' is too complex: {}", name, reasons.join(", "));
                        issues.push(Issue {
                            rule: Rule::TestComplexity,
                            severity: Severity::Warning,
                            message,
                            location: Location::new(
                                body.start_position().row + 1,
                                body.start_position().column + 1,
                            ),
                            suggestion: Some(
                                "Split into smaller tests or extract setup into helpers"
                                    .to_string(),
                            ),
                            fix: None,
                        });
                    }
                }
            }
        }

        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            Self::visit_tests(child, source, _tree, issues);
        }
    }
}

impl Default for TestComplexityRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for TestComplexityRule {
    fn name(&self) -> &'static str {
        "test-complexity"
    }

    fn analyze(&self, _tests: &[TestCase], source: &str, tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();
        let root = tree.root_node();
        Self::visit_tests(root, source, tree, &mut issues);
        issues
    }

    fn calculate_score(&self, _tests: &[TestCase], issues: &[Issue]) -> u8 {
        let n = issues
            .iter()
            .filter(|i| i.rule == Rule::TestComplexity)
            .count();
        (25i32 - (n as i32 * 3).min(25)).max(0) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_too_many_assertions() {
        let rule = TestComplexityRule::new();
        let source = (0..20)
            .map(|i| format!("expect({}).toBe({});", i, i))
            .collect::<Vec<_>>()
            .join("\n");
        let full = format!("it('big', () => {{\n{}\n}});", source);
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse(&full)
            .unwrap();
        let issues = rule.analyze(&[], &full, &tree);
        assert!(!issues.is_empty());
        assert!(issues.iter().any(|i| i.rule == Rule::TestComplexity));
    }

    #[test]
    fn negative_simple_test_no_issue() {
        let rule = TestComplexityRule::new();
        let source = "it('adds', () => { expect(1 + 1).toBe(2); });";
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse(source)
            .unwrap();
        let issues = rule.analyze(&[], source, &tree);
        assert!(issues.is_empty());
    }
}
