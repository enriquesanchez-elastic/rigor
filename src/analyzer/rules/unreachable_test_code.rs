//! Unreachable test code: code after return/throw in test body.

use super::AnalysisRule;
use crate::parser::{containing_test_body, node_to_location};
use crate::{Issue, Rule, Severity, TestCase};
use tree_sitter::{Node, Tree};

/// Rule for detecting unreachable code in tests
pub struct UnreachableTestCodeRule;

impl UnreachableTestCodeRule {
    pub fn new() -> Self {
        Self
    }

    fn visit_for_unreachable(node: Node, source: &str, tree: &Tree, issues: &mut Vec<Issue>) {
        if node.kind() == "return_statement" || node.kind() == "throw_statement" {
            if containing_test_body(node, tree, source).is_none() {
                return;
            }
            let parent = match node.parent() {
                Some(p) => p,
                None => return,
            };
            if parent.kind() != "statement_block" && parent.kind() != "block" {
                return;
            }
            let mut cursor = parent.walk();
            let mut seen_return_throw = false;
            for child in parent.named_children(&mut cursor) {
                if child.start_byte() == node.start_byte() && child.end_byte() == node.end_byte() {
                    seen_return_throw = true;
                    continue;
                }
                if seen_return_throw
                    && (child.kind() != "comment"
                        && child.kind() != "line_comment"
                        && child.kind() != "block_comment")
                {
                    let loc = node_to_location(child);
                    issues.push(Issue {
                        rule: Rule::UnreachableTestCode,
                        severity: Severity::Warning,
                        message: "Unreachable code after return/throw".to_string(),
                        location: loc,
                        suggestion: Some(
                            "Remove dead code or move it before the return/throw".to_string(),
                        ),
                        fix: None,
                    });
                    break;
                }
            }
            return;
        }

        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            Self::visit_for_unreachable(child, source, tree, issues);
        }
    }
}

impl Default for UnreachableTestCodeRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for UnreachableTestCodeRule {
    fn name(&self) -> &'static str {
        "unreachable-test-code"
    }

    fn analyze(&self, _tests: &[TestCase], source: &str, tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();
        let root = tree.root_node();
        Self::visit_for_unreachable(root, source, tree, &mut issues);
        issues
    }

    fn calculate_score(&self, _tests: &[TestCase], issues: &[Issue]) -> u8 {
        let n = issues
            .iter()
            .filter(|i| i.rule == Rule::UnreachableTestCode)
            .count();
        (25i32 - (n as i32 * 3).min(15)).max(0) as u8
    }
}
