//! Type assertion abuse: overuse of type assertions (as Type) instead of real checks.

use super::AnalysisRule;
use crate::{Issue, TestCase};
use tree_sitter::Tree;

/// Rule for detecting type assertion overuse in tests
pub struct TypeAssertionAbuseRule;

impl TypeAssertionAbuseRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TypeAssertionAbuseRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for TypeAssertionAbuseRule {
    fn name(&self) -> &'static str {
        "type-assertion-abuse"
    }

    fn analyze(&self, _tests: &[TestCase], _source: &str, _tree: &Tree) -> Vec<Issue> {
        // TODO: detect expect(x as Type) or excessive type casts without value checks
        vec![]
    }

    fn calculate_score(&self, _tests: &[TestCase], issues: &[Issue]) -> u8 {
        let n = issues
            .iter()
            .filter(|i| i.rule == crate::Rule::TypeAssertionAbuse)
            .count();
        (25i32 - (n as i32 * 2).min(10)).max(0) as u8
    }
}
