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
        
        // Collect actual values for better suggestions
        diversity.collected_numbers = unique_numbers.iter().map(|s| s.to_string()).collect();
        diversity.collected_strings = unique_strings.iter()
            .take(5) // Limit to avoid huge lists
            .map(|s| s.to_string())
            .collect();

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
    /// Track the actual numeric values found (for better suggestions)
    collected_numbers: Vec<String>,
    /// Track the actual string values found (for better suggestions)
    collected_strings: Vec<String>,
}

impl ValueDiversity {
    /// Suggest missing edge cases based on collected values
    fn suggest_missing_edge_cases(&self) -> String {
        let mut suggestions = Vec::new();
        
        if self.has_numbers {
            let nums_preview: Vec<&str> = self.collected_numbers.iter()
                .take(5)
                .map(|s| s.as_str())
                .collect();
            
            let mut missing = Vec::new();
            if !self.has_zero {
                missing.push("0");
            }
            if !self.has_negative {
                missing.push("-1");
            }
            
            if !missing.is_empty() {
                let collected = if nums_preview.is_empty() {
                    String::new()
                } else {
                    format!("Uses: [{}]. ", nums_preview.join(", "))
                };
                suggestions.push(format!("{}Consider adding: {}", collected, missing.join(", ")));
            }
        }
        
        if self.has_strings && !self.has_empty_string {
            let strs_preview: Vec<&str> = self.collected_strings.iter()
                .take(3)
                .map(|s| s.as_str())
                .collect();
            
            if !strs_preview.is_empty() {
                suggestions.push(format!("Uses strings like: [{}]. Consider adding: ''", strs_preview.join(", ")));
            } else {
                suggestions.push("Consider adding empty string ''".to_string());
            }
        }
        
        suggestions.join("; ")
    }
    
    /// Format the collected values for display
    fn format_collected_values(&self, kind: &str) -> String {
        match kind {
            "number" => {
                let nums: Vec<&str> = self.collected_numbers.iter()
                    .take(5)
                    .map(|s| s.as_str())
                    .collect();
                if nums.is_empty() {
                    String::new()
                } else if nums.len() == 1 {
                    format!(" (only uses: {})", nums[0])
                } else {
                    format!(" (uses: [{}])", nums.join(", "))
                }
            }
            "string" => {
                let strs: Vec<&str> = self.collected_strings.iter()
                    .take(3)
                    .map(|s| s.as_str())
                    .collect();
                if strs.is_empty() {
                    String::new()
                } else if strs.len() == 1 {
                    format!(" (only uses: {})", strs[0])
                } else {
                    format!(" (uses: [{}])", strs.join(", "))
                }
            }
            _ => String::new()
        }
    }
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

        // Check for limited variety with specific value reporting
        if !diversity.has_zero && diversity.has_numbers {
            let values_info = diversity.format_collected_values("number");
            issues.push(Issue {
                rule: Rule::LimitedInputVariety,
                severity: Severity::Info,
                message: format!("Tests use numbers but don't test with 0{}", values_info),
                location: Location::new(1, 1),
                suggestion: Some(format!("Add test cases with value 0. {}", diversity.suggest_missing_edge_cases())),
            });
        }

        if !diversity.has_negative && diversity.has_numbers && diversity.unique_number_count > 2 {
            let values_info = diversity.format_collected_values("number");
            issues.push(Issue {
                rule: Rule::LimitedInputVariety,
                severity: Severity::Info,
                message: format!("Tests use numbers but don't test negative values{}", values_info),
                location: Location::new(1, 1),
                suggestion: Some("Add test cases with negative numbers like -1".to_string()),
            });
        }

        if !diversity.has_empty_string && diversity.has_strings && diversity.unique_string_count > 2
        {
            let values_info = diversity.format_collected_values("string");
            issues.push(Issue {
                rule: Rule::LimitedInputVariety,
                severity: Severity::Info,
                message: format!("Tests use strings but don't test empty strings{}", values_info),
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
            let value = diversity.collected_numbers.first()
                .map(|s| s.as_str())
                .unwrap_or("?");
            let mut missing = Vec::new();
            if !diversity.has_zero && value != "0" {
                missing.push("0");
            }
            if !diversity.has_negative {
                missing.push("-1");
            }
            missing.push("larger values");
            
            issues.push(Issue {
                rule: Rule::LimitedInputVariety,
                severity: Severity::Warning,
                message: format!("All tests use the same numeric value: {}", value),
                location: Location::new(1, 1),
                suggestion: Some(format!("Vary test input values. Consider adding: {}", missing.join(", "))),
            });
        }

        if diversity.unique_string_count == 1 && diversity.has_strings && tests.len() > 3 {
            let value = diversity.collected_strings.first()
                .map(|s| {
                    if s.len() > 20 {
                        format!("{}...", &s[..17])
                    } else {
                        s.clone()
                    }
                })
                .unwrap_or_else(|| "?".to_string());
            
            issues.push(Issue {
                rule: Rule::LimitedInputVariety,
                severity: Severity::Warning,
                message: format!("All tests use the same string value: {}", value),
                location: Location::new(1, 1),
                suggestion: Some("Vary test input strings. Consider adding: '', special characters, long strings".to_string()),
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
