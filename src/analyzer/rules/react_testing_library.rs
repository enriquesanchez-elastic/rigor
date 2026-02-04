//! React Testing Library best practice rules

use super::AnalysisRule;
use crate::{Issue, Location, Rule, Severity, TestCase};
use tree_sitter::Tree;

/// Rule for React Testing Library best practices (only runs when RTL is detected)
pub struct ReactTestingLibraryRule;

impl ReactTestingLibraryRule {
    pub fn new() -> Self {
        Self
    }

    /// Check if the file uses React Testing Library (imports)
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

    fn analyze(&self, _tests: &[TestCase], source: &str, _tree: &Tree) -> Vec<Issue> {
        if !Self::uses_rtl(source) {
            return vec![];
        }

        let mut issues = Vec::new();

        for (zero_indexed, line) in source.lines().enumerate() {
            let line_no = zero_indexed + 1;
            let trimmed = line.trim();

            if trimmed.starts_with("//") || trimmed.starts_with("/*") {
                continue;
            }

            // container.querySelector - prefer screen.getByRole
            if trimmed.contains("container.querySelector(")
                || trimmed.contains("container.querySelector (")
            {
                let col = line.find("querySelector").unwrap_or(0) + 1;
                issues.push(Issue {
                    rule: Rule::RtlPreferScreen,
                    severity: Severity::Warning,
                    message: "Avoid container.querySelector - prefer screen.getByRole or screen.getByLabelText for accessibility".to_string(),
                    location: Location::new(line_no, col),
                    suggestion: Some(
                        "Use screen.getByRole('button', { name: 'Submit' }) or screen.getByLabelText('Email') instead".to_string(),
                    ),
                });
            }

            // getByTestId as primary query - prefer semantic queries
            if (trimmed.contains("getByTestId(") || trimmed.contains("getByTestId ("))
                && !trimmed.contains("getByRole")
                && !trimmed.contains("getByLabelText")
            {
                let col = line.find("getByTestId").unwrap_or(0) + 1;
                issues.push(Issue {
                    rule: Rule::RtlPreferSemantic,
                    severity: Severity::Info,
                    message: "getByTestId is a last resort - prefer getByRole, getByLabelText, or getByText for user-facing behavior".to_string(),
                    location: Location::new(line_no, col),
                    suggestion: Some(
                        "Use getByRole('button', { name: '...' }) or getByLabelText('...') when possible".to_string(),
                    ),
                });
            }

            // fireEvent - prefer userEvent
            if trimmed.contains("fireEvent.") && !trimmed.contains("userEvent") {
                let col = line.find("fireEvent").unwrap_or(0) + 1;
                issues.push(Issue {
                    rule: Rule::RtlPreferUserEvent,
                    severity: Severity::Info,
                    message: "Prefer userEvent over fireEvent for more realistic user interactions".to_string(),
                    location: Location::new(line_no, col),
                    suggestion: Some(
                        "Use @testing-library/user-event: userEvent.click(element) instead of fireEvent.click(element)".to_string(),
                    ),
                });
            }
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
