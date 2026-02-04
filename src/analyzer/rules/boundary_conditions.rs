//! Boundary conditions analysis rule

use super::AnalysisRule;
use crate::parser::SourceFileParser;
use crate::{Issue, Location, Rule, Severity, TestCase};
use tree_sitter::Tree;

/// Rule for analyzing boundary condition coverage
pub struct BoundaryConditionsRule {
    source_content: Option<String>,
    source_tree: Option<Tree>,
}

impl BoundaryConditionsRule {
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

    /// Check if tests cover a specific boundary value
    fn tests_cover_boundary(tests: &[TestCase], value: &str, operator: &str) -> bool {
        // Parse the numeric value
        let num: f64 = match value.parse() {
            Ok(n) => n,
            Err(_) => return false,
        };

        // Determine boundary values to test based on operator
        let boundary_values: Vec<f64> = match operator {
            ">=" | "<=" => vec![num, num - 1.0, num + 1.0],
            ">" | "<" => vec![num, num - 1.0, num + 1.0],
            "==" | "===" => vec![num, num - 1.0, num + 1.0],
            _ => vec![num],
        };

        // Check if any test mentions these values
        for test in tests {
            let test_text = format!("{} {:?}", test.name, test.assertions);
            for boundary in &boundary_values {
                if test_text.contains(&boundary.to_string()) {
                    return true;
                }
            }
        }

        false
    }
}

impl Default for BoundaryConditionsRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for BoundaryConditionsRule {
    fn name(&self) -> &'static str {
        "boundary-conditions"
    }

    fn analyze(&self, tests: &[TestCase], _source: &str, _tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();

        // If we have source file, analyze boundary conditions
        if let (Some(source_content), Some(source_tree)) =
            (&self.source_content, &self.source_tree)
        {
            let parser = SourceFileParser::new(source_content);
            let boundaries = parser.extract_boundary_conditions(source_tree);

            for boundary in boundaries {
                if let Some(ref value) = boundary.value {
                    if !Self::tests_cover_boundary(tests, value, &boundary.operator) {
                        let context = if boundary.context.is_empty() {
                            String::new()
                        } else {
                            format!(" in '{}'", boundary.context)
                        };

                        issues.push(Issue {
                            rule: Rule::MissingBoundaryTest,
                            severity: Severity::Warning,
                            message: format!(
                                "Boundary condition '{} {}'{} may not be fully tested",
                                boundary.operator, value, context
                            ),
                            location: Location::new(1, 1),
                            suggestion: Some(format!(
                                "Add tests: expect(fn({})).toBe(expected); expect(fn({})).toBe(expected); expect(fn({})).toBe(expected)",
                                value.parse::<f64>().unwrap_or(0.0) - 1.0,
                                value,
                                value.parse::<f64>().unwrap_or(0.0) + 1.0
                            )),
                        });
                    }
                }
            }
        }

        // Also check test file for hardcoded edge values
        let edge_values = ["0", "-1", "1", "null", "undefined", "''", "[]", "{}"];
        let mut has_edge_value_tests = false;

        for test in tests {
            for assertion in &test.assertions {
                for edge in edge_values {
                    if assertion.raw.contains(edge) {
                        has_edge_value_tests = true;
                        break;
                    }
                }
            }
        }

        // If no edge value tests and we have tests, add an info issue
        if !has_edge_value_tests && !tests.is_empty() && tests.len() < 5 {
            issues.push(Issue {
                rule: Rule::MissingBoundaryTest,
                severity: Severity::Info,
                message: "Tests may not cover edge cases like 0, empty values, or boundaries"
                    .to_string(),
                location: Location::new(1, 1),
                suggestion: Some(
                    "Add tests: expect(fn(0)).toBe(...); expect(fn('')).toBe(...); expect(fn(null)).toThrow()".to_string(),
                ),
            });
        }

        issues
    }

    fn calculate_score(&self, tests: &[TestCase], issues: &[Issue]) -> u8 {
        if tests.is_empty() {
            return 0;
        }

        let mut score: i32 = 25;

        // Count missing boundary tests
        let missing_boundaries = issues
            .iter()
            .filter(|i| i.rule == Rule::MissingBoundaryTest && i.severity == Severity::Warning)
            .count();

        // Deduct for missing boundary tests (-3 each, max -15)
        score -= (missing_boundaries as i32 * 3).min(15);

        // Deduct for general lack of edge case testing (-5)
        let has_edge_case_warning = issues.iter().any(|i| {
            i.rule == Rule::MissingBoundaryTest
                && i.severity == Severity::Info
                && i.message.contains("edge cases")
        });

        if has_edge_case_warning {
            score -= 5;
        }

        // Bonus for tests that explicitly test boundaries
        let boundary_keywords = ["edge", "boundary", "limit", "max", "min", "zero", "empty"];
        let boundary_tests = tests
            .iter()
            .filter(|t| {
                let name_lower = t.name.to_lowercase();
                boundary_keywords.iter().any(|k| name_lower.contains(k))
            })
            .count();

        if boundary_tests > 0 {
            score += (boundary_tests as i32 * 2).min(5);
        }

        score.clamp(0, 25) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Assertion, AssertionKind, Location};

    fn make_test(name: &str, assertions: Vec<Assertion>) -> TestCase {
        TestCase {
            name: name.to_string(),
            location: Location::new(1, 1),
            is_async: false,
            is_skipped: false,
            assertions,
            describe_block: None,
        }
    }

    #[test]
    fn test_boundary_detection() {
        let tests = vec![make_test(
            "handles age 18",
            vec![Assertion {
                kind: AssertionKind::ToBe,
                quality: AssertionKind::ToBe.quality(),
                location: Location::new(1, 1),
                raw: "expect(isAdult(18)).toBe(true)".to_string(),
            }],
        )];

        assert!(BoundaryConditionsRule::tests_cover_boundary(
            &tests, "18", ">="
        ));
    }

    #[test]
    fn test_missing_edge_case_detection() {
        let tests = vec![make_test(
            "basic test",
            vec![Assertion {
                kind: AssertionKind::ToBe,
                quality: AssertionKind::ToBe.quality(),
                location: Location::new(1, 1),
                raw: "expect(x).toBe(42)".to_string(),
            }],
        )];

        let rule = BoundaryConditionsRule::new();
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("test")
            .unwrap();
        let issues = rule.analyze(&tests, "", &tree);

        assert!(issues
            .iter()
            .any(|i| i.message.contains("edge cases")));
    }
}
