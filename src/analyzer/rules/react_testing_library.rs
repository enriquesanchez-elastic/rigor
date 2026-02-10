//! React Testing Library best practice rules.
//! Uses tree-sitter to find querySelector/getByTestId/fireEvent calls and avoid false positives in comments/strings.

use super::AnalysisRule;
use crate::parser::{
    find_call_expressions, is_inside_comment_range, is_inside_string_literal_range,
};
use crate::{Issue, Rule, Severity, TestCase};
use tree_sitter::Tree;

/// Rule for React Testing Library best practices (only runs when RTL is detected)
pub struct ReactTestingLibraryRule;

impl ReactTestingLibraryRule {
    pub fn new() -> Self {
        Self
    }

    fn uses_rtl(source: &str) -> bool {
        source.contains("@testing-library/react")
            || source.contains("@testing-library/dom")
            || source.contains("from '@testing-library/react'")
            || source.contains("from \"@testing-library/react\"")
    }
}

impl Default for ReactTestingLibraryRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for ReactTestingLibraryRule {
    fn name(&self) -> &'static str {
        "react-testing-library"
    }

    fn analyze(&self, _tests: &[TestCase], source: &str, tree: &Tree) -> Vec<Issue> {
        if !Self::uses_rtl(source) {
            return vec![];
        }

        let mut issues = Vec::new();
        let root = tree.root_node();

        for call in find_call_expressions(tree, source, "container.querySelector") {
            if is_inside_comment_range(call.start_byte, call.end_byte, source) {
                continue;
            }
            if is_inside_string_literal_range(call.start_byte, call.end_byte, root) {
                continue;
            }
            issues.push(Issue {
                rule: Rule::RtlPreferScreen,
                severity: Severity::Warning,
                message: "Avoid container.querySelector - prefer screen.getByRole or screen.getByLabelText for accessibility".to_string(),
                location: call.location.clone(),
                suggestion: Some(
                    "Use screen.getByRole('button', { name: 'Submit' }) or screen.getByLabelText('Email') instead".to_string(),
                ),
                fix: None,
            });
        }

        for call in find_call_expressions(tree, source, "getByTestId") {
            if is_inside_comment_range(call.start_byte, call.end_byte, source) {
                continue;
            }
            if is_inside_string_literal_range(call.start_byte, call.end_byte, root) {
                continue;
            }
            let line_src = source
                .lines()
                .nth(call.location.line.saturating_sub(1))
                .unwrap_or("");
            if line_src.contains("getByRole") || line_src.contains("getByLabelText") {
                continue;
            }
            issues.push(Issue {
                rule: Rule::RtlPreferSemantic,
                severity: Severity::Info,
                message: "getByTestId is a last resort - prefer getByRole, getByLabelText, or getByText for user-facing behavior".to_string(),
                location: call.location.clone(),
                suggestion: Some(
                    "Use getByRole('button', { name: '...' }) or getByLabelText('...') when possible".to_string(),
                ),
                fix: None,
            });
        }

        for call in find_call_expressions(tree, source, "fireEvent") {
            if is_inside_comment_range(call.start_byte, call.end_byte, source) {
                continue;
            }
            if is_inside_string_literal_range(call.start_byte, call.end_byte, root) {
                continue;
            }
            let line_src = source
                .lines()
                .nth(call.location.line.saturating_sub(1))
                .unwrap_or("");
            if line_src.contains("userEvent") {
                continue;
            }
            issues.push(Issue {
                rule: Rule::RtlPreferUserEvent,
                severity: Severity::Info,
                message: "Prefer userEvent over fireEvent for more realistic user interactions".to_string(),
                location: call.location.clone(),
                suggestion: Some(
                    "Use @testing-library/user-event: userEvent.click(element) instead of fireEvent.click(element)".to_string(),
                ),
                fix: None,
            });
        }

        issues
    }

    fn calculate_score(&self, _tests: &[TestCase], issues: &[Issue]) -> u8 {
        let count = issues
            .iter()
            .filter(|i| {
                matches!(
                    i.rule,
                    Rule::RtlPreferScreen | Rule::RtlPreferSemantic | Rule::RtlPreferUserEvent
                )
            })
            .count();
        let mut score: i32 = 25;
        score -= (count as i32 * 2).min(12);
        score.clamp(0, 25) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Issue, Location, Severity, TestCase};

    fn make_empty_tests() -> Vec<TestCase> {
        vec![]
    }

    #[test]
    fn positive_detects_query_selector_with_rtl() {
        let rule = ReactTestingLibraryRule::new();
        let source = r#"
        import { render } from '@testing-library/react';
        const { container } = render(<App />);
        const btn = container.querySelector('.button');
        "#;
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse(source)
            .unwrap();
        let issues = rule.analyze(&make_empty_tests(), source, &tree);
        assert!(!issues.is_empty());
        assert!(issues.iter().any(|i| i.rule == Rule::RtlPreferScreen));
    }

    #[test]
    fn negative_no_rtl_import_no_issues() {
        let rule = ReactTestingLibraryRule::new();
        let source = "it('works', () => { expect(1).toBe(1); });";
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse(source)
            .unwrap();
        let issues = rule.analyze(&make_empty_tests(), source, &tree);
        assert!(issues.is_empty());
    }

    #[test]
    fn score_decreases_with_issues() {
        let rule = ReactTestingLibraryRule::new();
        let tests = make_empty_tests();
        let one_issue = vec![Issue {
            rule: Rule::RtlPreferScreen,
            severity: Severity::Warning,
            message: "test".to_string(),
            location: Location::new(1, 1),
            suggestion: None,
            fix: None,
        }];
        assert_eq!(rule.calculate_score(&tests, &one_issue), 23);
    }
}
