//! Excessive setup: beforeEach/beforeAll doing too much.

use super::AnalysisRule;
use crate::parser::{global_query_cache, QueryId, TypeScriptParser};
use crate::{Issue, Location, Rule, Severity, TestCase};
use tree_sitter::{Node, Tree};

const SETUP_STATEMENT_THRESHOLD: usize = 15;

/// Rule for detecting excessive test setup
pub struct ExcessiveSetupRule;

impl ExcessiveSetupRule {
    pub fn new() -> Self {
        Self
    }

    fn count_statements(node: Node) -> usize {
        let kind = node.kind();
        let one = if kind == "expression_statement"
            || kind == "lexical_declaration"
            || kind == "variable_declaration"
            || kind == "return_statement"
            || kind == "throw_statement"
            || kind == "if_statement"
            || kind == "for_statement"
            || kind == "while_statement"
        {
            1
        } else {
            0
        };
        let mut sum = one;
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            sum += Self::count_statements(child);
        }
        sum
    }
}

impl Default for ExcessiveSetupRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for ExcessiveSetupRule {
    fn name(&self) -> &'static str {
        "excessive-setup"
    }

    fn analyze(&self, _tests: &[TestCase], source: &str, tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();
        let lang = TypeScriptParser::language();
        let cache = global_query_cache();

        if let Ok(matches) = cache.run_cached_query(source, tree, &lang, QueryId::BeforeAfterHook) {
            let root = tree.root_node();
            for caps in matches {
                let fn_name = caps
                    .iter()
                    .find(|c| c.name == "fn")
                    .map(|c| c.text.as_str());
                if fn_name != Some("beforeEach") && fn_name != Some("beforeAll") {
                    continue;
                }
                let call_cap = match caps.iter().find(|c| c.name == "call") {
                    Some(c) => c,
                    None => continue,
                };
                let call_node =
                    match root.descendant_for_byte_range(call_cap.start_byte, call_cap.end_byte) {
                        Some(n) => n,
                        None => continue,
                    };
                let args = match call_node.child_by_field_name("arguments") {
                    Some(a) => a,
                    None => continue,
                };
                let mut cursor = args.walk();
                let children: Vec<Node> = args.named_children(&mut cursor).collect();
                if children.is_empty() {
                    continue;
                }
                let callback = children[0];
                let count = Self::count_statements(callback);
                if count > SETUP_STATEMENT_THRESHOLD {
                    let (line, col) = call_cap.start_point;
                    issues.push(Issue {
                        rule: Rule::ExcessiveSetup,
                        severity: Severity::Info,
                        message: format!(
                            "{} has {} statements - consider extracting helpers or reducing setup",
                            fn_name.unwrap_or("Hook"),
                            count
                        ),
                        location: Location::new(line, col),
                        suggestion: Some(
                            "Extract setup into named functions or shared fixtures".to_string(),
                        ),
                        fix: None,
                    });
                }
            }
        }

        issues
    }

    fn calculate_score(&self, _tests: &[TestCase], issues: &[Issue]) -> u8 {
        let n = issues
            .iter()
            .filter(|i| i.rule == Rule::ExcessiveSetup)
            .count();
        (25i32 - (n as i32 * 2).min(10)).max(0) as u8
    }
}
