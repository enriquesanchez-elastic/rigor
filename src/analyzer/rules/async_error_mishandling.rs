//! Async error mishandling: async error path not properly tested (rejects, catch).

use super::AnalysisRule;
use crate::parser::{global_query_cache, QueryId, TypeScriptParser};
use crate::{Issue, Location, Rule, Severity, TestCase};
use tree_sitter::Tree;

/// Rule for detecting async tests that don't properly test error paths
pub struct AsyncErrorMishandlingRule;

impl AsyncErrorMishandlingRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AsyncErrorMishandlingRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for AsyncErrorMishandlingRule {
    fn name(&self) -> &'static str {
        "async-error-mishandling"
    }

    fn analyze(&self, _tests: &[TestCase], source: &str, tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();
        let lang = TypeScriptParser::language();
        let cache = global_query_cache();
        let root = tree.root_node();

        if let Ok(matches) = cache.run_cached_query(source, tree, &lang, QueryId::ExpectCall) {
            for caps in matches {
                let obj = caps
                    .iter()
                    .find(|c| c.name == "obj")
                    .map(|c| c.text.as_str());
                let fn_name = caps
                    .iter()
                    .find(|c| c.name == "fn")
                    .map(|c| c.text.as_str());
                if fn_name != Some("expect") && obj != Some("expect") {
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
                let mut current = call_node;
                let mut has_rejects = false;
                while let Some(parent) = current.parent() {
                    if parent.kind() == "await_expression" {
                        break;
                    }
                    if parent.kind() == "member_expression" {
                        if let Some(prop) = parent.child_by_field_name("property") {
                            let text = prop.utf8_text(source.as_bytes()).unwrap_or("");
                            if text == "rejects" {
                                has_rejects = true;
                                break;
                            }
                        }
                    }
                    current = parent;
                }
                if has_rejects {
                    let mut up = call_node;
                    let mut under_await = false;
                    while let Some(p) = up.parent() {
                        if p.kind() == "await_expression" {
                            under_await = true;
                            break;
                        }
                        up = p;
                    }
                    if !under_await {
                        let (line, col) = call_cap.start_point;
                        issues.push(Issue {
                            rule: Rule::AsyncErrorMishandling,
                            severity: Severity::Warning,
                            message: "expect().rejects used without await - use 'await expect(...).rejects.toThrow()'".to_string(),
                            location: Location::new(line, col),
                            suggestion: Some(
                                "Add await: await expect(asyncFn()).rejects.toThrow(Error)".to_string(),
                            ),
                            fix: None,
                        });
                    }
                }
            }
        }

        issues
    }

    fn calculate_score(&self, _tests: &[TestCase], issues: &[Issue]) -> u8 {
        let n = issues
            .iter()
            .filter(|i| i.rule == Rule::AsyncErrorMishandling)
            .count();
        (25i32 - (n as i32 * 3).min(15)).max(0) as u8
    }
}
