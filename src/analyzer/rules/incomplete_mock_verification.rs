//! Incomplete mock verification: mock is used but not fully verified (e.g. toHaveBeenCalledWith).

use super::AnalysisRule;
use crate::{Issue, TestCase};
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

    fn analyze(&self, _tests: &[TestCase], _source: &str, _tree: &Tree) -> Vec<Issue> {
        // TODO: detect jest.spyOn/mock without toHaveBeenCalledWith or toHaveBeenCalled
        vec![]
    }

    fn calculate_score(&self, _tests: &[TestCase], issues: &[Issue]) -> u8 {
        let n = issues
            .iter()
            .filter(|i| i.rule == crate::Rule::IncompleteMockVerification)
            .count();
        (25i32 - (n as i32 * 3).min(15)).max(0) as u8
    }
}
