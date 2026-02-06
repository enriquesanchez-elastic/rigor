//! Missing cleanup: afterEach or reset mocks not used when needed.

use super::AnalysisRule;
use crate::{Issue, TestCase};
use tree_sitter::Tree;

/// Rule for detecting missing test cleanup
pub struct MissingCleanupRule;

impl MissingCleanupRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MissingCleanupRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for MissingCleanupRule {
    fn name(&self) -> &'static str {
        "missing-cleanup"
    }

    fn analyze(&self, _tests: &[TestCase], _source: &str, _tree: &Tree) -> Vec<Issue> {
        // TODO: when mocks or mutable state exist, suggest afterEach / mockClear
        vec![]
    }

    fn calculate_score(&self, _tests: &[TestCase], issues: &[Issue]) -> u8 {
        let n = issues
            .iter()
            .filter(|i| i.rule == crate::Rule::MissingCleanup)
            .count();
        (25i32 - (n as i32 * 2).min(10)).max(0) as u8
    }
}
