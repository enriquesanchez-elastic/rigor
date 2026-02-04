//! Input variety analysis rule

use super::AnalysisRule;
use crate::{Issue, Location, Rule, Severity, TestCase};
use std::collections::HashSet;
use tree_sitter::{Node, Tree};

/// Rule for analyzing input variety in tests
pub struct InputVarietyRule;

impl InputVarietyRule {
    pub fn new() -> Self {
        Self
    }

    /// Extract test values used in assertions
    fn extract_test_values(source: &str, tree: &Tree) -> Vec<TestValue> {
        let mut values = Vec::new();
        Self::visit_for_values(tree.root_node(), source, &mut values);
        values
    }

    fn visit_for_values(node: Node, source: &str, values: &mut Vec<TestValue>) {
        // Look for literal values in expect() calls and test data
        match node.kind() {
            "string" | "template_string" => {
                let text = node.utf8_text(source.as_bytes()).unwrap_or("");
                let location = Location::new(
                    node.start_position().row + 1,
                    node.start_position().column + 1,
                );
                values.push(TestValue {
                    kind: ValueKind::String,
                    raw: text.to_string(),
                    location,
                });
            }
            "number" => {
                let text = node.utf8_text(source.as_bytes()).unwrap_or("");
                let location = Location::new(
                    node.start_position().row + 1,
                    node.start_position().column + 1,
                );
                values.push(TestValue {
                    kind: ValueKind::Number,
                    raw: text.to_string(),
                    location,
                });
            }
            "true" | "false" => {
                let text = node.utf8_text(source.as_bytes()).unwrap_or("");
                let location = Location::new(
                    node.start_position().row + 1,
                    node.start_position().column + 1,
                );
                values.push(TestValue {
                    kind: ValueKind::Boolean,
                    raw: text.to_string(),
                    location,
                });
            }
            "null" => {
                let location = Location::new(
                    node.start_position().row + 1,
                    node.start_position().column + 1,
                );
                values.push(TestValue {
                    kind: ValueKind::Null,
                    raw: "null".to_string(),
                    location,
                });
            }
            "array" => {
                // Check for empty arrays
                let text = node.utf8_text(source.as_bytes()).unwrap_or("");
                if text == "[]" {
                    let location = Location::new(
                        node.start_position().row + 1,
                        node.start_position().column + 1,
                    );
                    values.push(TestValue {
                        kind: ValueKind::EmptyArray,
                        raw: "[]".to_string(),
                        location,
                    });
                }
            }
            "object" => {
                // Check for empty objects
                let text = node.utf8_text(source.as_bytes()).unwrap_or("");
                if text.trim() == "{}" {
                    let location = Location::new(
                        node.start_position().row + 1,
                        node.start_position().column + 1,
                    );
                    values.push(TestValue {
                        kind: ValueKind::EmptyObject,
                        raw: "{}".to_string(),
                        location,
                    });
                }
            }
            _ => {}
        }

        // Recurse
        for child in node.named_children(&mut node.walk()) {
            Self::visit_for_values(child, source, values);
        }
    }

    fn analyze_value_diversity(values: &[TestValue]) -> ValueDiversity {
        let mut diversity = ValueDiversity::default();

        let mut unique_strings: HashSet<&str> = HashSet::new();
        let mut unique_numbers: HashSet<&str> = HashSet::new();

        for value in values {
            match value.kind {
                ValueKind::String => {
                    diversity.has_strings = true;
                    unique_strings.insert(&value.raw);
                    if value.raw == "''" || value.raw == "\"\"" || value.raw == "``" {
                        diversity.has_empty_string = true;
                    }
                }
                ValueKind::Number => {
                    diversity.has_numbers = true;
                    unique_numbers.insert(&value.raw);
                    if value.raw == "0" {
                        diversity.has_zero = true;
                    }
                    if value.raw.starts_with('-') {
                        diversity.has_negative = true;
                    }
                }
                ValueKind::Boolean => {
                    diversity.has_booleans = true;
                }
                ValueKind::Null => {
                    diversity.has_null = true;
                }
                ValueKind::EmptyArray => {
                    diversity.has_empty_array = true;
                }
                ValueKind::EmptyObject => {
                    diversity.has_empty_object = true;
                }
            }
        }

        diversity.unique_string_count = unique_strings.len();
        diversity.unique_number_count = unique_numbers.len();

        diversity
    }
}

impl Default for InputVarietyRule {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
struct TestValue {
    kind: ValueKind,
    raw: String,
    location: Location,
}

#[derive(Debug, PartialEq)]
enum ValueKind {
    String,
    Number,
    Boolean,
    Null,
    EmptyArray,
    EmptyObject,
}

#[derive(Debug, Default)]
struct ValueDiversity {
    has_strings: bool,
    has_numbers: bool,
    has_booleans: bool,
    has_null: bool,
    has_zero: bool,
    has_negative: bool,
    has_empty_string: bool,
    has_empty_array: bool,
    has_empty_object: bool,
    unique_string_count: usize,
    unique_number_count: usize,
}

impl AnalysisRule for InputVarietyRule {
    fn name(&self) -> &'static str {
        "input-variety"
    }

    fn analyze(&self, tests: &[TestCase], source: &str, tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();

        if tests.is_empty() {
            return issues;
        }

        let values = Self::extract_test_values(source, tree);
        let diversity = Self::analyze_value_diversity(&values);

        // Check for limited variety
        if !diversity.has_zero && diversity.has_numbers {
            issues.push(Issue {
                rule: Rule::LimitedInputVariety,
                severity: Severity::Info,
                message: "Tests use numbers but don't test with 0".to_string(),
                location: Location::new(1, 1),
                suggestion: Some("Add test cases with value 0".to_string()),
            });
        }

        if !diversity.has_negative && diversity.has_numbers && diversity.unique_number_count > 2 {
            issues.push(Issue {
                rule: Rule::LimitedInputVariety,
                severity: Severity::Info,
                message: "Tests use numbers but don't test negative values".to_string(),
                location: Location::new(1, 1),
                suggestion: Some("Add test cases with negative numbers".to_string()),
            });
        }

        if !diversity.has_empty_string && diversity.has_strings && diversity.unique_string_count > 2
        {
            issues.push(Issue {
                rule: Rule::LimitedInputVariety,
                severity: Severity::Info,
                message: "Tests use strings but don't test empty strings".to_string(),
                location: Location::new(1, 1),
                suggestion: Some("Add test cases with empty string ''".to_string()),
            });
        }

        if !diversity.has_null && tests.len() > 3 {
            issues.push(Issue {
                rule: Rule::LimitedInputVariety,
                severity: Severity::Info,
                message: "Tests don't include null value testing".to_string(),
                location: Location::new(1, 1),
                suggestion: Some("Consider adding test cases with null values".to_string()),
            });
        }

        // Check for hardcoded values that look like real data
        for value in &values {
            if value.kind == ValueKind::String {
                let trimmed = value.raw.trim_matches(|c| c == '"' || c == '\'' || c == '`');
                // Check for patterns that look like real emails, names, etc.
                if trimmed.contains('@') && trimmed.contains('.') && trimmed.len() > 10 {
                    issues.push(Issue {
                        rule: Rule::HardcodedValues,
                        severity: Severity::Info,
                        message: format!(
                            "Hardcoded email-like value: {}",
                            if value.raw.len() > 30 {
                                format!("{}...", &value.raw[..27])
                            } else {
                                value.raw.clone()
                            }
                        ),
                        location: value.location.clone(),
                        suggestion: Some(
                            "Consider using test fixtures or faker library for test data"
                                .to_string(),
                        ),
                    });
                }
            }
        }

        // Check if tests only use a very limited set of values
        if diversity.unique_number_count == 1 && tests.len() > 3 {
            issues.push(Issue {
                rule: Rule::LimitedInputVariety,
                severity: Severity::Warning,
                message: "All tests use the same numeric value".to_string(),
                location: Location::new(1, 1),
                suggestion: Some("Vary test input values to cover more cases".to_string()),
            });
        }

        if diversity.unique_string_count == 1 && diversity.has_strings && tests.len() > 3 {
            issues.push(Issue {
                rule: Rule::LimitedInputVariety,
                severity: Severity::Warning,
                message: "All tests use the same string value".to_string(),
                location: Location::new(1, 1),
                suggestion: Some("Vary test input strings to cover more cases".to_string()),
            });
        }

        issues
    }

    fn calculate_score(&self, tests: &[TestCase], issues: &[Issue]) -> u8 {
        if tests.is_empty() {
            return 0;
        }

        let mut score: i32 = 25;

        // Count variety issues
        let variety_warnings = issues
            .iter()
            .filter(|i| {
                (i.rule == Rule::LimitedInputVariety || i.rule == Rule::HardcodedValues)
                    && i.severity == Severity::Warning
            })
            .count();

        let variety_info = issues
            .iter()
            .filter(|i| {
                (i.rule == Rule::LimitedInputVariety || i.rule == Rule::HardcodedValues)
                    && i.severity == Severity::Info
            })
            .count();

        // Deduct for variety issues
        score -= (variety_warnings as i32 * 4).min(12);
        score -= (variety_info as i32 * 2).min(8);

        score.clamp(0, 25) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::TypeScriptParser;

    #[test]
    fn test_detect_values() {
        let source = r#"
            describe('test', () => {
                it('uses numbers', () => {
                    expect(add(1, 2)).toBe(3);
                });
            });
        "#;

        let mut parser = TypeScriptParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let values = InputVarietyRule::extract_test_values(source, &tree);

        assert!(values.iter().any(|v| v.kind == ValueKind::Number));
    }

    #[test]
    fn test_diversity_analysis() {
        let values = vec![
            TestValue {
                kind: ValueKind::Number,
                raw: "1".to_string(),
                location: Location::new(1, 1),
            },
            TestValue {
                kind: ValueKind::Number,
                raw: "2".to_string(),
                location: Location::new(1, 1),
            },
            TestValue {
                kind: ValueKind::Number,
                raw: "0".to_string(),
                location: Location::new(1, 1),
            },
        ];

        let diversity = InputVarietyRule::analyze_value_diversity(&values);
        assert!(diversity.has_zero);
        assert!(diversity.has_numbers);
        assert_eq!(diversity.unique_number_count, 3);
    }
}
