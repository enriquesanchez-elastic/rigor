//! Redundant test: test duplicates another test's coverage.

use super::AnalysisRule;
use crate::{Issue, TestCase};
use tree_sitter::Tree;

/// Rule for detecting redundant tests
pub struct RedundantTestRule;

impl RedundantTestRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RedundantTestRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for RedundantTestRule {
    fn name(&self) -> &'static str {
        "redundant-test"
    }

    fn analyze(&self, _tests: &[TestCase], _source: &str, _tree: &Tree) -> Vec<Issue> {
        // TODO: heuristic overlap (same assertions, same inputs)
        vec![]
    }

    fn calculate_score(&self, _tests: &[TestCase], issues: &[Issue]) -> u8 {
        let n = issues
            .iter()
            .filter(|i| i.rule == crate::Rule::RedundantTest)
            .count();
        (25i32 - (n as i32 * 2).min(10)).max(0) as u8
    }
}
