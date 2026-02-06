//! Async error mishandling: async error path not properly tested (rejects, catch).

use super::AnalysisRule;
use crate::{Issue, TestCase};
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

    fn analyze(&self, _tests: &[TestCase], _source: &str, _tree: &Tree) -> Vec<Issue> {
        // TODO: detect async tests that should use rejects.toThrow but don't
        vec![]
    }

    fn calculate_score(&self, _tests: &[TestCase], issues: &[Issue]) -> u8 {
        let n = issues
            .iter()
            .filter(|i| i.rule == crate::Rule::AsyncErrorMishandling)
            .count();
        (25i32 - (n as i32 * 3).min(15)).max(0) as u8
    }
}
