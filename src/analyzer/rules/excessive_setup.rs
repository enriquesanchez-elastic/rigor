//! Excessive setup: beforeEach/beforeAll doing too much.

use super::AnalysisRule;
use crate::{Issue, TestCase};
use tree_sitter::Tree;

/// Rule for detecting excessive test setup
pub struct ExcessiveSetupRule;

impl ExcessiveSetupRule {
    pub fn new() -> Self {
        Self
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

    fn analyze(&self, _tests: &[TestCase], _source: &str, _tree: &Tree) -> Vec<Issue> {
        // TODO: flag beforeEach with many statements or complex logic
        vec![]
    }

    fn calculate_score(&self, _tests: &[TestCase], issues: &[Issue]) -> u8 {
        let n = issues
            .iter()
            .filter(|i| i.rule == crate::Rule::ExcessiveSetup)
            .count();
        (25i32 - (n as i32 * 2).min(10)).max(0) as u8
    }
}
