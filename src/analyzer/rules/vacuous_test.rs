//! Vacuous test: test that always passes or does not meaningfully verify behavior.

use super::trivial_assertion::TrivialAssertionRule;
use super::AnalysisRule;
use crate::parser::{find_assertions_in_body, node_line_count};
use crate::{Issue, Location, Rule, Severity, TestCase};
use tree_sitter::{Node, Tree};

/// Rule for detecting vacuous tests
pub struct VacuousTestRule;

impl VacuousTestRule {
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
            if matches!(obj.as_deref(), Some("it") | Some("test")) {
                return true;
            }
        }
        false
    }

    fn visit_tests(node: Node, source: &str, tree: &Tree, issues: &mut Vec<Issue>) {
        if Self::is_test_call(node, source.as_bytes()) {
            if let Some(args) = node.child_by_field_name("arguments") {
                let mut cursor = args.walk();
                let children: Vec<Node> = args.named_children(&mut cursor).collect();
                if children.len() >= 2 {
                    let name_node = children[0];
                    let body = children[1];
                    let name = Self::node_text(name_node, source.as_bytes());
                    let name = name.trim_matches(|c| c == '"' || c == '\'');

                    let assertions = find_assertions_in_body(body, source.as_bytes());
                    if assertions.is_empty() {
                        let line_count = node_line_count(body);
                        if line_count <= 2 {
                            issues.push(Issue {
                                rule: Rule::VacuousTest,
                                severity: Severity::Warning,
                                message: format!("Test '{}' has no assertions", name),
                                location: Location::new(
                                    body.start_position().row + 1,
                                    body.start_position().column + 1,
                                ),
                                suggestion: Some("Add assertions to verify behavior".to_string()),
                                fix: None,
                            });
                        }
                    } else {
                        let all_trivial = assertions.iter().all(|expect_node| {
                            TrivialAssertionRule::expect_is_trivial(
                                tree,
                                source,
                                expect_node.start_byte(),
                                expect_node.end_byte(),
                            )
                        });
                        if all_trivial {
                            issues.push(Issue {
                                rule: Rule::VacuousTest,
                                severity: Severity::Error,
                                message: format!(
                                    "Test '{}' only has trivial assertions — it does not verify real behavior",
                                    name
                                ),
                                location: Location::new(
                                    body.start_position().row + 1,
                                    body.start_position().column + 1,
                                ),
                                suggestion: Some(
                                    "Replace with assertions on the result of the code under test".to_string(),
                                ),
                                fix: None,
                            });
                        }
                    }
                }
            }
        }

        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            Self::visit_tests(child, source, tree, issues);
        }
    }
}

impl Default for VacuousTestRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for VacuousTestRule {
    fn name(&self) -> &'static str {
        "vacuous-test"
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
            .filter(|i| i.rule == Rule::VacuousTest)
            .count();
        (25i32 - (n as i32 * 5).min(25)).max(0) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_empty_test() {
        let rule = VacuousTestRule::new();
        let source = "it('does nothing', () => {});";
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse(source)
            .unwrap();
        let issues = rule.analyze(&[], source, &tree);
        assert!(!issues.is_empty());
        assert!(issues.iter().any(|i| i.rule == Rule::VacuousTest));
    }

    #[test]
    fn flags_all_trivial_assertions() {
        let rule = VacuousTestRule::new();
        let source = "it('tautology', () => { expect(1).toBe(1); expect(2).toBe(2); });";
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse(source)
            .unwrap();
        let issues = rule.analyze(&[], source, &tree);
        assert!(!issues.is_empty());
    }

    #[test]
    fn negative_meaningful_test_no_issue() {
        let rule = VacuousTestRule::new();
        let source = "it('adds', () => { expect(1 + 1).toBe(2); });";
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse(source)
            .unwrap();
        let issues = rule.analyze(&[], source, &tree);
        assert!(issues.is_empty());
    }
}
