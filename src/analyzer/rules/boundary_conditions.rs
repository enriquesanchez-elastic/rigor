//! Boundary conditions analysis rule

use super::AnalysisRule;
use crate::parser::SourceFileParser;
use crate::{Issue, Location, Rule, Severity, TestCase};
use regex::Regex;
use std::collections::HashMap;
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

    /// Returns true if `num` appears as a standalone integer in `text`
    /// (not adjacent to another digit on either side).
    fn is_standalone_number(text: &str, num: i64) -> bool {
        let s = num.to_string();
        let tbytes = text.as_bytes();
        let nbytes = s.as_bytes();
        if nbytes.len() > tbytes.len() {
            return false;
        }
        let mut i = 0;
        while i + nbytes.len() <= tbytes.len() {
            if tbytes[i..i + nbytes.len()] == *nbytes {
                let before_ok = i == 0 || !tbytes[i - 1].is_ascii_digit();
                let after_ok =
                    i + nbytes.len() == tbytes.len() || !tbytes[i + nbytes.len()].is_ascii_digit();
                if before_ok && after_ok {
                    return true;
                }
            }
            i += 1;
        }
        false
    }

    /// Check if tests cover a specific boundary value using standalone-number matching.
    fn tests_cover_boundary(tests: &[TestCase], value: &str, _operator: &str) -> bool {
        let num: f64 = match value.parse() {
            Ok(n) => n,
            Err(_) => return false,
        };
        let targets: [i64; 3] = [num as i64, (num - 1.0) as i64, (num + 1.0) as i64];
        for test in tests {
            for &t in &targets {
                if Self::is_standalone_number(&test.name, t) {
                    return true;
                }
                for assertion in &test.assertions {
                    if Self::is_standalone_number(&assertion.raw, t) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Heuristic: detect functions tested with only a single non-edge numeric value.
    /// For example, `validateAge(25)` but never 0, 17, 18, 19 suggests missing boundary tests.
    /// This works without source file access — purely from test content.
    fn detect_single_value_functions(tests: &[TestCase]) -> Vec<Issue> {
        // Extract fn_name(number) patterns from assertion raw text
        let call_re = Regex::new(r"(\w+)\((-?\d+(?:\.\d+)?)\)").unwrap();
        // Also match multi-arg calls to catch the first arg: fn(5, 0, 10)
        let multi_arg_re = Regex::new(r"(\w+)\((-?\d+(?:\.\d+)?)\s*,").unwrap();

        // Collect numeric args per function name
        let mut fn_values: HashMap<String, Vec<f64>> = HashMap::new();
        let mut fn_location: HashMap<String, Location> = HashMap::new();

        // Skip assertion matchers themselves (expect, toBe, etc.)
        let skip_names = [
            "expect",
            "toBe",
            "toEqual",
            "toStrictEqual",
            "toBeDefined",
            "toBeUndefined",
            "toBeNull",
            "toBeTruthy",
            "toBeFalsy",
            "toThrow",
            "toContain",
            "toMatch",
            "toHaveLength",
            "toBeGreaterThan",
            "toBeGreaterThanOrEqual",
            "toBeLessThan",
            "toBeLessThanOrEqual",
            "toHaveProperty",
            "toHaveBeenCalledTimes",
            "toHaveBeenCalledWith",
            "toMatchSnapshot",
            "toBeInstanceOf",
            "toBeCloseTo",
            "toHaveBeenNthCalledWith",
            "toHaveTextContent",
            "toBeInTheDocument",
            "toHaveAttribute",
        ];

        for test in tests {
            for assertion in &test.assertions {
                for re in [&call_re, &multi_arg_re] {
                    for cap in re.captures_iter(&assertion.raw) {
                        let fn_name = cap[1].to_string();
                        if skip_names.contains(&fn_name.as_str()) {
                            continue;
                        }
                        if let Ok(val) = cap[2].parse::<f64>() {
                            fn_values.entry(fn_name.clone()).or_default().push(val);
                            fn_location
                                .entry(fn_name)
                                .or_insert_with(|| assertion.location.clone());
                        }
                    }
                }
            }
        }

        let mut issues = Vec::new();
        let edge_values: &[f64] = &[-1.0, 0.0, 1.0];

        for (fn_name, values) in &fn_values {
            // De-duplicate
            let mut unique: Vec<f64> = values.clone();
            unique.sort_by(|a, b| a.partial_cmp(b).unwrap());
            unique.dedup();

            // If only 1 unique value and it's not an edge value, flag it.
            // Use generic suggestion (no source here) — actual boundaries come from source code.
            if unique.len() == 1 && !edge_values.contains(&unique[0]) {
                let val = unique[0] as i64;
                let location = fn_location
                    .get(fn_name)
                    .cloned()
                    .unwrap_or_else(|| Location::new(1, 1));

                issues.push(Issue {
                    rule: Rule::MissingBoundaryTest,
                    severity: Severity::Warning,
                    message: format!(
                        "'{}' is only tested with value {} — consider testing boundary values \
                         (e.g. min, max, or thresholds from the source)",
                        fn_name, val
                    ),
                    location,
                    suggestion: Some(
                        "Add boundary tests from source (e.g. expect(fn(threshold)).toBe(expected)). Consider testing min, max, and edge values.".to_string(),
                    ),
                    fix: None,
                });
            }
        }

        issues
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

        // If we have source file, analyze boundary conditions from source
        if let (Some(source_content), Some(source_tree)) = (&self.source_content, &self.source_tree)
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

                        let (v_lo, v, v_hi) = (
                            value.parse::<f64>().unwrap_or(0.0) - 1.0,
                            value.clone(),
                            value.parse::<f64>().unwrap_or(0.0) + 1.0,
                        );
                        let fn_placeholder = if boundary.context.is_empty() {
                            "fn".to_string()
                        } else {
                            boundary.context.clone()
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
                                "Add tests: expect({}({})).toBe(expected); expect({}({})).toBe(expected); expect({}({})).toBe(expected)",
                                fn_placeholder, v_lo, fn_placeholder, v, fn_placeholder, v_hi
                            )),
                            fix: None,
                        });
                    }
                }
            }
        }

        // Heuristic: detect functions tested with only a single numeric value
        // (works without source file — purely from test content)
        let single_value_issues = Self::detect_single_value_functions(tests);
        issues.extend(single_value_issues);

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
        if !has_edge_value_tests && !tests.is_empty() {
            issues.push(Issue {
                rule: Rule::MissingBoundaryTest,
                severity: Severity::Info,
                message: "Tests may not cover edge cases like 0, empty values, or boundaries"
                    .to_string(),
                location: Location::new(1, 1),
                suggestion: Some(
                    "Add tests: expect(fn(0)).toBe(...); expect(fn('')).toBe(...); expect(fn(null)).toThrow()".to_string(),
                ),
                fix: None,
            });
        }

        issues
    }

    fn calculate_score(&self, tests: &[TestCase], issues: &[Issue]) -> u8 {
        if tests.is_empty() {
            return 0;
        }
        let total_tests = tests.len().max(1);
        let mut score: i32 = 25;

        let missing = issues
            .iter()
            .filter(|i| i.rule == Rule::MissingBoundaryTest && i.severity == Severity::Warning)
            .count();
        let edge_warn = issues.iter().any(|i| {
            i.rule == Rule::MissingBoundaryTest
                && i.severity == Severity::Info
                && i.message.contains("edge cases")
        });

        score -= ((missing as f32 / total_tests as f32).min(1.0) * 15.0) as i32;
        if edge_warn {
            score -= 5;
        }

        // Bonus: tests with boundary-keyword names
        let boundary_keywords = ["edge", "boundary", "limit", "max", "min", "zero", "empty"];
        let boundary_tests = tests
            .iter()
            .filter(|t| {
                let n = t.name.to_lowercase();
                boundary_keywords.iter().any(|k| n.contains(k))
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

    fn make_assertion(raw: &str) -> Assertion {
        Assertion {
            kind: AssertionKind::ToBe,
            quality: AssertionKind::ToBe.quality(),
            location: Location::new(1, 1),
            raw: raw.to_string(),
        }
    }

    #[test]
    fn test_boundary_detection() {
        let tests = vec![make_test(
            "handles age 18",
            vec![make_assertion("expect(isAdult(18)).toBe(true)")],
        )];

        assert!(BoundaryConditionsRule::tests_cover_boundary(
            &tests, "18", ">="
        ));
    }

    #[test]
    fn test_boundary_not_detected_when_not_mentioned() {
        let tests = vec![make_test(
            "basic test",
            vec![make_assertion("expect(x).toBe(42)")],
        )];

        assert!(!BoundaryConditionsRule::tests_cover_boundary(
            &tests, "18", ">="
        ));
    }

    #[test]
    fn test_missing_edge_case_detection() {
        let tests = vec![make_test(
            "basic test",
            vec![make_assertion("expect(x).toBe(42)")],
        )];

        let rule = BoundaryConditionsRule::new();
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("test")
            .unwrap();
        let issues = rule.analyze(&tests, "", &tree);

        assert!(issues.iter().any(|i| i.message.contains("edge cases")));
    }

    // --- detect_single_value_functions heuristic ---

    #[test]
    fn single_value_function_flagged() {
        // validateAge(25) only — no boundary values tested
        let tests = vec![make_test(
            "validateAge only tests one value",
            vec![make_assertion("expect(validateAge(25)).toBe(true)")],
        )];

        let issues = BoundaryConditionsRule::detect_single_value_functions(&tests);
        assert!(
            !issues.is_empty(),
            "should flag validateAge(25) as single-value"
        );
        assert!(issues[0].message.contains("validateAge"));
        assert!(issues[0].message.contains("25"));
        assert!(issues[0].rule == Rule::MissingBoundaryTest);
    }

    #[test]
    fn multiple_values_not_flagged() {
        // validateAge tested with 17, 18, 19 — boundary coverage is good
        let tests = vec![
            make_test(
                "rejects age 17",
                vec![make_assertion("expect(validateAge(17)).toBe(false)")],
            ),
            make_test(
                "accepts age 18",
                vec![make_assertion("expect(validateAge(18)).toBe(true)")],
            ),
            make_test(
                "accepts age 19",
                vec![make_assertion("expect(validateAge(19)).toBe(true)")],
            ),
        ];

        let issues = BoundaryConditionsRule::detect_single_value_functions(&tests);
        assert!(
            !issues.iter().any(|i| i.message.contains("validateAge")),
            "should NOT flag validateAge when tested with multiple values"
        );
    }

    #[test]
    fn edge_value_zero_not_flagged() {
        // Testing with 0 is itself an edge value — no flag
        let tests = vec![make_test(
            "handles zero",
            vec![make_assertion("expect(calculate(0)).toBe(0)")],
        )];

        let issues = BoundaryConditionsRule::detect_single_value_functions(&tests);
        assert!(
            !issues.iter().any(|i| i.message.contains("calculate")),
            "should NOT flag when the single value is an edge value (0)"
        );
    }

    #[test]
    fn assertion_matchers_not_flagged_as_functions() {
        // toBe(42) should not be flagged — toBe is an assertion matcher, not a function under test
        let tests = vec![make_test(
            "basic assertion",
            vec![make_assertion("expect(result).toBe(42)")],
        )];

        let issues = BoundaryConditionsRule::detect_single_value_functions(&tests);
        assert!(
            !issues.iter().any(|i| i.message.contains("toBe")),
            "should NOT flag assertion matchers like toBe"
        );
        assert!(
            !issues.iter().any(|i| i.message.contains("expect")),
            "should NOT flag expect"
        );
    }

    #[test]
    fn multi_arg_function_first_arg_flagged() {
        // clamp(5, 0, 10) — 5 is the only value for clamp's first arg
        let tests = vec![make_test(
            "clamp middle",
            vec![make_assertion("expect(clamp(5, 0, 10)).toBe(5)")],
        )];

        let issues = BoundaryConditionsRule::detect_single_value_functions(&tests);
        assert!(
            issues.iter().any(|i| i.message.contains("clamp")),
            "should flag clamp(5, ...) as single-value"
        );
    }

    #[test]
    fn integration_boundary_heuristic_fires_without_source() {
        // Full analyze path without source file — the heuristic should still detect issues
        let tests = vec![
            make_test(
                "validateAge only tests one value",
                vec![make_assertion("expect(validateAge(25)).toBe(true)")],
            ),
            make_test(
                "clamp only tests middle",
                vec![make_assertion("expect(clamp(5, 0, 10)).toBe(5)")],
            ),
        ];

        let rule = BoundaryConditionsRule::new(); // no source
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("test")
            .unwrap();
        let issues = rule.analyze(&tests, "", &tree);

        let boundary_warnings: Vec<_> = issues
            .iter()
            .filter(|i| i.rule == Rule::MissingBoundaryTest && i.severity == Severity::Warning)
            .collect();

        assert!(
            !boundary_warnings.is_empty(),
            "should detect missing boundary tests from test content alone, got issues: {:?}",
            issues.iter().map(|i| &i.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn edge_case_warning_fires_for_large_file() {
        // 6 tests, none use 0/-1/null/empty — must still warn
        // Using values that don't contain characters from edge values {0, -1, 1, null, undefined, '', [], {}}
        // 2, 3, 4, 5, 6, 7 all avoid those substrings
        let tests: Vec<TestCase> = (2..=7)
            .map(|i| {
                make_test(
                    &format!("test {i}"),
                    vec![make_assertion(&format!("expect(fn({})).toBe(true)", i))],
                )
            })
            .collect();

        let rule = BoundaryConditionsRule::new();
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("test")
            .unwrap();
        let issues = rule.analyze(&tests, "test", &tree);

        assert!(
            issues.iter().any(|i| {
                i.rule == Rule::MissingBoundaryTest
                    && i.severity == Severity::Info
                    && i.message.contains("edge cases")
            }),
            "should warn about missing edge cases even when tests.len() >= 5. Issues: {:#?}",
            issues
        );
    }

    #[test]
    fn boundary_check_does_not_match_18_inside_180() {
        // 180 must NOT satisfy a >= 18 boundary check
        let tests = vec![make_test(
            "handles large age",
            vec![make_assertion("expect(isAdult(180)).toBe(true)")],
        )];
        assert!(
            !BoundaryConditionsRule::tests_cover_boundary(&tests, "18", ">="),
            "value 180 must not satisfy boundary check for 18"
        );
    }

    #[test]
    fn boundary_check_matches_exact_standalone_number() {
        let tests = vec![make_test(
            "handles age 18",
            vec![make_assertion("expect(isAdult(18)).toBe(true)")],
        )];
        assert!(
            BoundaryConditionsRule::tests_cover_boundary(&tests, "18", ">="),
            "value 18 must satisfy boundary check for 18"
        );
    }
}
