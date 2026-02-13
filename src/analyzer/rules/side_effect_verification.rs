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

    /// Check if the mutation target is asserted: it must appear inside expect(...),
    /// e.g. expect(cart.items).toContain(...).
    ///
    /// Also handles `this.property` targets from constructors: if the target is
    /// `this.name`, we also match `instance.name`, `error.name`, etc. — any
    /// `*.property` pattern inside an expect() argument.
    fn test_verifies_mutation_target(&self, test_source: &str, target: &str) -> bool {
        let source_lower = test_source.to_lowercase();
        let target_lower = target.to_lowercase();
        if target_lower.is_empty() {
            return false;
        }

        // Extract the property name if target is "this.property"
        let this_property = target_lower
            .strip_prefix("this.")
            .map(|prop| format!(".{}", prop));

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
                        // Direct match: target appears verbatim in expect()
                        if arg.contains(&target_lower) {
                            return true;
                        }
                        // Constructor match: for "this.prop", also match "*.prop"
                        // e.g., this.name → instance.name, error.name, etc.
                        if let Some(ref dot_prop) = this_property {
                            if arg.contains(dot_prop.as_str()) {
                                return true;
                            }
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
                    fix: None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Issue, Severity, TestCase};

    #[test]
    fn negative_no_source_returns_empty() {
        let rule = SideEffectVerificationRule::new();
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("it('test', () => {});")
            .unwrap();
        let tests = vec![TestCase {
            name: "addItem".to_string(),
            location: crate::Location::new(1, 1),
            is_async: false,
            is_skipped: false,
            assertions: vec![],
            describe_block: None,
        }];
        let test_source = "";
        let issues = rule.analyze(&tests, test_source, &tree);
        assert!(issues.is_empty());
    }

    #[test]
    fn positive_with_source_mutation_not_asserted() {
        let source_content = r#"
        function appendItem(arr: number[]): number[] {
            arr.push(1);
            return arr;
        }
        "#;
        let mut parser = crate::parser::TypeScriptParser::new().unwrap();
        let source_tree = parser.parse(source_content).unwrap();
        let rule =
            SideEffectVerificationRule::new().with_source(source_content.to_string(), source_tree);
        let tests = vec![TestCase {
            name: "appendItem".to_string(),
            location: crate::Location::new(2, 1),
            is_async: false,
            is_skipped: false,
            assertions: vec![],
            describe_block: None,
        }];
        let test_source = "const out = appendItem([]); expect(out).toEqual([1]);";
        let tree = parser.parse(test_source).unwrap();
        let issues = rule.analyze(&tests, test_source, &tree);
        assert!(
            issues.iter().any(|i| i.rule == Rule::SideEffectNotVerified),
            "expected SideEffectNotVerified when mutation target not in expect()"
        );
    }

    #[test]
    fn constructor_property_assertion_counts_as_verified() {
        // When source has: this.name = 'ParseError' (target: "this.name")
        // and test has: expect(instance.name).toBe('ParseError')
        // the mutation should be considered verified.
        let rule = SideEffectVerificationRule::new();
        let test_source =
            "const instance = new ParseError('msg'); expect(instance.name).toBe('ParseError');";
        assert!(
            rule.test_verifies_mutation_target(test_source, "this.name"),
            "expect(instance.name) should verify mutation target 'this.name'"
        );
    }

    #[test]
    fn this_property_not_verified_without_expect() {
        let rule = SideEffectVerificationRule::new();
        let test_source = "const instance = new ParseError('msg'); console.log(instance.name);";
        assert!(
            !rule.test_verifies_mutation_target(test_source, "this.name"),
            "console.log should not count as verification"
        );
    }

    #[test]
    fn score_decreases_with_issues() {
        let rule = SideEffectVerificationRule::new();
        let tests: Vec<TestCase> = vec![];
        let zero_issues: Vec<Issue> = vec![];
        let one_issue = vec![Issue {
            rule: Rule::SideEffectNotVerified,
            severity: Severity::Warning,
            message: "test".to_string(),
            location: crate::Location::new(1, 1),
            suggestion: None,
            fix: None,
        }];
        assert_eq!(rule.calculate_score(&tests, &zero_issues), 25);
        assert_eq!(rule.calculate_score(&tests, &one_issue), 20);
    }
}
