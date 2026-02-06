//! Implementation coupling: test is tightly coupled to implementation details.

use super::AnalysisRule;
use crate::{Issue, TestCase};
use tree_sitter::Tree;

/// Rule for detecting implementation-coupled tests
pub struct ImplementationCouplingRule;

impl ImplementationCouplingRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ImplementationCouplingRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for ImplementationCouplingRule {
    fn name(&self) -> &'static str {
        "implementation-coupling"
    }

    fn analyze(&self, _tests: &[TestCase], _source: &str, _tree: &Tree) -> Vec<Issue> {
        // TODO: detect tests that assert on internal state or private methods
        vec![]
    }

    fn calculate_score(&self, _tests: &[TestCase], issues: &[Issue]) -> u8 {
        let n = issues
            .iter()
            .filter(|i| i.rule == crate::Rule::ImplementationCoupling)
            .count();
        (25i32 - (n as i32 * 2).min(15)).max(0) as u8
    }
}
