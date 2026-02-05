//! Side effect verification analysis rule
//!
//! Flags when source functions mutate state (e.g. array push, property assign) but tests
//! only verify the return value and do not assert on the mutated state.

use super::AnalysisRule;
use crate::parser::{MutationKind, SourceFileParser};
use crate::{Issue, Location, Rule, Severity, TestCase};
use tree_sitter::Tree;

/// Test file location for the function (first test that references it)
fn test_location_for_function(tests: &[TestCase], fn_name: &str) -> Location {
    let fn_lower = fn_name.to_lowercase();
    for test in tests {
        if test.name.to_lowercase().contains(&fn_lower)
            || test
                .assertions
                .iter()
                .any(|a| a.raw.to_lowercase().contains(&fn_lower))
        {
            return test.location.clone();
        }
    }
    tests
        .first()
        .map(|t| t.location.clone())
        .unwrap_or_else(|| Location::new(1, 1))
}

/// Rule for analyzing whether tests verify side effects
pub struct SideEffectVerificationRule {
    source_content: Option<String>,
    source_tree: Option<Tree>,
}

impl SideEffectVerificationRule {
    pub fn new() -> Self {
        Self {
            source_content: None,
            source_tree: None,
        }
    }

    /// Set the corresponding source file content for analysis
    pub fn with_source(mut self, content: String, tree: Tree) -> Self {
        self.source_content = Some(content);
        self.source_tree = Some(tree);
        self
    }

    /// Check if the mutation target is asserted: it must appear inside expect(...), e.g. expect(cart.items).toContain(...)
    fn test_verifies_mutation_target(&self, test_source: &str, target: &str) -> bool {
        let source_lower = test_source.to_lowercase();
        let target_lower = target.to_lowercase();
        if target_lower.is_empty() {
            return false;
        }
        // Find each expect( ... ) and see if target appears in the argument
        let mut i = 0;
        while let Some(start) = source_lower[i..].find("expect(") {
            let expect_start = i + start + "expect(".len();
            let mut depth = 1u32;
            let mut j = expect_start;
            let bytes = source_lower.as_bytes();
            while j < bytes.len() && depth > 0 {
                let c = bytes[j] as char;
                if c == '(' {
                    depth += 1;
                } else if c == ')' {
                    depth -= 1;
                    if depth == 0 {
                        let arg = &source_lower[expect_start..j];
                        if arg.contains(&target_lower) {
                            return true;
                        }
                        break;
                    }
                }
                j += 1;
            }
            i = expect_start + 1;
        }
        false
    }

    fn mutation_suggestion(kind: &MutationKind) -> &'static str {
        match kind {
            MutationKind::ArrayPush => "expect(mutatedArray).toContain(addedItem)",
            MutationKind::ArrayPop => "expect(array).toHaveLength(n - 1)",
            MutationKind::ArraySplice => "expect(array).toHaveLength(...) or toContain(...)",
            MutationKind::ArraySort | MutationKind::ArrayReverse => {
                "expect(array).toEqual([...expectedOrder])"
            }
            MutationKind::ArrayShift | MutationKind::ArrayUnshift => {
                "expect(array).toHaveLength(...)"
            }
            MutationKind::PropertyAssign | MutationKind::VariableAssign => {
                "expect(obj.prop).toBe(expected) or toEqual(...)"
            }
        }
    }
}

impl Default for SideEffectVerificationRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for SideEffectVerificationRule {
    fn name(&self) -> &'static str {
        "side-effect-verification"
    }

    fn analyze(&self, tests: &[TestCase], source: &str, _tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();

        if tests.is_empty() {
            return issues;
        }

        if let (Some(ref source_content), Some(ref source_tree)) =
            (&self.source_content, &self.source_tree)
        {
            let parser = SourceFileParser::new(source_content);
            let details = parser.extract_function_details(source_tree);

            for func in details {
                let unverified: Vec<_> = func
                    .mutations
                    .iter()
                    .filter(|m| !self.test_verifies_mutation_target(source, &m.target))
                    .collect();
                if unverified.is_empty() {
                    continue;
                }

                let location = test_location_for_function(tests, &func.name);
                let kind_desc = |k: &MutationKind| match k {
                    MutationKind::ArrayPush => "array push",
                    MutationKind::ArrayPop => "array pop",
                    MutationKind::ArraySplice => "array splice",
                    MutationKind::ArraySort => "array sort",
                    MutationKind::ArrayReverse => "array reverse",
                    MutationKind::ArrayShift => "array shift",
                    MutationKind::ArrayUnshift => "array unshift",
                    MutationKind::PropertyAssign => "property assignment",
                    MutationKind::VariableAssign => "variable assignment",
                };
                let list = unverified
                    .iter()
                    .map(|m| format!("{} ({})", m.target, kind_desc(&m.kind)))
                    .collect::<Vec<_>>()
                    .join("; ");
                let first = unverified[0];
                let suggestion = format!(
                    "After calling {}, assert the mutation. For '{}': {}",
                    func.name,
                    first.target,
                    Self::mutation_suggestion(&first.kind)
                );
                issues.push(Issue {
                    rule: Rule::SideEffectNotVerified,
                    severity: Severity::Warning,
                    message: format!(
                        "Function '{}' mutates state ({} unverified: {}) but tests don't assert on it",
                        func.name, unverified.len(), list
                    ),
                    location,
                    suggestion: Some(suggestion),
                });
            }
        }

        issues
    }

    fn calculate_score(&self, _tests: &[TestCase], issues: &[Issue]) -> u8 {
        let side_effect_issues = issues
            .iter()
            .filter(|i| i.rule == Rule::SideEffectNotVerified)
            .count();
        let deduction = (side_effect_issues as i32 * 5).min(25);
        (25 - deduction).max(0) as u8
    }
}
