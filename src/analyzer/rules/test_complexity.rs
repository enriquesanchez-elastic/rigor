//! Test complexity: flags tests that are too complex (high cyclomatic complexity or too many assertions).

use super::AnalysisRule;
use crate::{Issue, TestCase};
use tree_sitter::Tree;

/// Rule for detecting overly complex tests
pub struct TestComplexityRule;

impl TestComplexityRule {
    pub fn new() -> Self {
        Self
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

    fn analyze(&self, _tests: &[TestCase], _source: &str, _tree: &Tree) -> Vec<Issue> {
        // TODO: use tree-sitter to count branches/assertions per test; flag when over threshold
        vec![]
    }

    fn calculate_score(&self, _tests: &[TestCase], issues: &[Issue]) -> u8 {
        let n = issues
            .iter()
            .filter(|i| i.rule == crate::Rule::TestComplexity)
            .count();
        (25i32 - (n as i32 * 3).min(25)).max(0) as u8
    }
}
