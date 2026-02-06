//! Vacuous test: test that always passes or does not meaningfully verify behavior.

use super::AnalysisRule;
use crate::{Issue, TestCase};
use tree_sitter::Tree;

/// Rule for detecting vacuous tests
pub struct VacuousTestRule;

impl VacuousTestRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for VacuousTestRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for VacuousTestRule {
    fn name(&self) -> &'static str {
        "vacuous-test"
    }

    fn analyze(&self, _tests: &[TestCase], _source: &str, _tree: &Tree) -> Vec<Issue> {
        // TODO: overlap with trivial_assertion; flag tests that only have tautologies
        vec![]
    }

    fn calculate_score(&self, _tests: &[TestCase], issues: &[Issue]) -> u8 {
        let n = issues
            .iter()
            .filter(|i| i.rule == crate::Rule::VacuousTest)
            .count();
        (25i32 - (n as i32 * 5).min(25)).max(0) as u8
    }
}
