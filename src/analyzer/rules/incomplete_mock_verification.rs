//! Incomplete mock verification: mock is used but not fully verified (e.g. toHaveBeenCalledWith).

use super::AnalysisRule;
use crate::parser::{containing_test_body, global_query_cache, QueryId, TypeScriptParser};
use crate::{Issue, Location, Rule, Severity, TestCase};
use tree_sitter::Tree;

/// Rule for detecting mocks that are not properly verified
pub struct IncompleteMockVerificationRule;

impl IncompleteMockVerificationRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for IncompleteMockVerificationRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for IncompleteMockVerificationRule {
    fn name(&self) -> &'static str {
        "incomplete-mock-verification"
    }

    fn analyze(&self, _tests: &[TestCase], source: &str, tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();
        let lang = TypeScriptParser::language();
        let cache = global_query_cache();

        if let Ok(matches) = cache.run_cached_query(source, tree, &lang, QueryId::MockCall) {
            let root = tree.root_node();
            for caps in matches {
                let obj = caps
                    .iter()
                    .find(|c| c.name == "obj")
                    .map(|c| c.text.as_str());
                let prop = caps
                    .iter()
                    .find(|c| c.name == "prop")
                    .map(|c| c.text.as_str());
                if obj != Some("jest") && obj != Some("vi") {
                    continue;
                }
                if prop != Some("spyOn") && prop != Some("fn") {
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
                let body = match containing_test_body(call_node, tree, source) {
                    Some(b) => b,
                    None => continue,
                };
                let body_slice = source.get(body.start_byte()..body.end_byte()).unwrap_or("");
                if body_slice.contains("toHaveBeenCalled")
                    || body_slice.contains("toHaveBeenCalledWith")
                    || body_slice.contains("toHaveBeenCalledTimes")
                {
                    continue;
                }
                let (line, col) = call_cap.start_point;
                issues.push(Issue {
                    rule: Rule::IncompleteMockVerification,
                    severity: Severity::Warning,
                    message: format!(
                        "Mock ({}.) is not verified - add expect(mock).toHaveBeenCalled() or toHaveBeenCalledWith(...)",
                        prop.unwrap_or("?")
                    ),
                    location: Location::new(line, col),
                    suggestion: Some(
                        "Verify mock was called: expect(mock).toHaveBeenCalledWith(expectedArgs)".to_string(),
                    ),
                    fix: None,
                });
            }
        }

        issues
    }

    fn calculate_score(&self, _tests: &[TestCase], issues: &[Issue]) -> u8 {
        let n = issues
            .iter()
            .filter(|i| i.rule == Rule::IncompleteMockVerification)
            .count();
        (25i32 - (n as i32 * 3).min(15)).max(0) as u8
    }
}
