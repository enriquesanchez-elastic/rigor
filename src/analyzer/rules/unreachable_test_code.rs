//! Unreachable test code: code after return/throw in test body.

use super::AnalysisRule;
use crate::{Issue, TestCase};
use tree_sitter::Tree;

/// Rule for detecting unreachable code in tests
pub struct UnreachableTestCodeRule;

impl UnreachableTestCodeRule {
    pub fn new() -> Self {
        Self
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

    fn analyze(&self, _tests: &[TestCase], _source: &str, _tree: &Tree) -> Vec<Issue> {
        // TODO: tree-sitter: find return/throw in test call body, flag code after
        vec![]
    }

    fn calculate_score(&self, _tests: &[TestCase], issues: &[Issue]) -> u8 {
        let n = issues
            .iter()
            .filter(|i| i.rule == crate::Rule::UnreachableTestCode)
            .count();
        (25i32 - (n as i32 * 3).min(15)).max(0) as u8
    }
}
