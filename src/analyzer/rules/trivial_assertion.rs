//! Trivial / nonsensical assertions: tests that don't meaningfully verify behavior.
//! Uses tree-sitter to compare expect(X) and matcher(Z) structurally; falls back to regex when needed.

use super::AnalysisRule;
use crate::parser::{containing_test_body, global_query_cache, QueryId, TypeScriptParser};
use crate::{Issue, Location, Rule, Severity, TestCase};
use regex::Regex;
use tree_sitter::Tree;

/// Rule that detects trivial or nonsensical assertions.
pub struct TrivialAssertionRule;

fn trivial_patterns() -> Vec<Regex> {
    [
        r"expect\(true\)\.to(Be|Equal|StrictEqual)\(true\)",
        r"expect\(false\)\.to(Be|Equal|StrictEqual)\(false\)",
        r"expect\(1\)\.to(Be|Equal|StrictEqual)\(1\)",
        r"expect\(0\)\.to(Be|Equal|StrictEqual)\(0\)",
        r"expect\(null\)\.to(Be|Equal|StrictEqual)\(null\)",
        r"expect\(undefined\)\.to(Be|Equal|StrictEqual)\(undefined\)",
    ]
    .iter()
    .map(|s| Regex::new(s).unwrap())
    .collect()
}

fn same_number_both_sides_re() -> Regex {
    Regex::new(r"expect\((\d+)\)\.to(be|equal|strictequal)\((\d+)\)").unwrap()
}

fn same_string_single_quote_re() -> Regex {
    Regex::new(r#"expect\('([^']*)'\)\.to(be|equal|strictequal)\('([^']*)'\)"#).unwrap()
}
fn same_string_double_quote_re() -> Regex {
    Regex::new(r#"expect\("([^"]*)"\)\.to(be|equal|strictequal)\("([^"]*)"\)"#).unwrap()
}

fn same_identifier_both_sides_re() -> Regex {
    Regex::new(r"expect\((\w+)\)\.to(equal|strictequal)\((\w+)\)").unwrap()
}

impl TrivialAssertionRule {
    pub fn new() -> Self {
        Self
    }

    fn is_trivial_literal(raw: &str) -> bool {
        let normalized: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
        trivial_patterns().iter().any(|re| re.is_match(&normalized))
    }

    fn same_number_both_sides(raw: &str) -> bool {
        let n: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
        let n_lower = n.to_lowercase();
        n_lower.contains("expect(1).tobe(1)")
            || n_lower.contains("expect(0).tobe(0)")
            || n_lower.contains("expect(1).toequal(1)")
            || n_lower.contains("expect(0).toequal(0)")
            || n_lower.contains("expect(true).tobe(true)")
            || n_lower.contains("expect(false).tobe(false)")
    }

    fn is_trivial_same_number(raw: &str) -> bool {
        let normalized: String = raw
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .to_lowercase();
        same_number_both_sides_re()
            .captures(&normalized)
            .and_then(|c| {
                let a = c.get(1)?.as_str();
                let b = c.get(3)?.as_str();
                if a == b {
                    Some(())
                } else {
                    None
                }
            })
            .is_some()
    }

    fn is_trivial_same_string(raw: &str) -> bool {
        let normalized: String = raw
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .to_lowercase();
        let check = |c: regex::Captures| {
            let left = c.get(1).map(|m| m.as_str());
            let right = c.get(3).map(|m| m.as_str());
            match (left, right) {
                (Some(a), Some(b)) if a == b => Some(()),
                _ => None,
            }
        };
        same_string_single_quote_re()
            .captures(&normalized)
            .and_then(check)
            .is_some()
            || same_string_double_quote_re()
                .captures(&normalized)
                .and_then(check)
                .is_some()
    }

    fn is_trivial_same_identifier(raw: &str) -> bool {
        let normalized: String = raw
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .to_lowercase();
        same_identifier_both_sides_re()
            .captures(&normalized)
            .and_then(|c| {
                let a = c.get(1)?.as_str();
                let b = c.get(3)?.as_str();
                if a == b {
                    Some(())
                } else {
                    None
                }
            })
            .is_some()
    }

    /// AST: compare two value nodes (left = expect arg, right = matcher arg). Returns true if same value.
    fn ast_values_equal(left: tree_sitter::Node, right: tree_sitter::Node, source: &[u8]) -> bool {
        let left_text = left.utf8_text(source).unwrap_or_default().trim();
        let right_text = right.utf8_text(source).unwrap_or_default().trim();
        match (left.kind(), right.kind()) {
            ("number", "number") => left_text == right_text,
            ("string", "string") | ("template_string", "template_string") => {
                left_text.trim_matches(|c| c == '"' || c == '\'' || c == '`')
                    == right_text.trim_matches(|c| c == '"' || c == '\'' || c == '`')
            }
            ("true", "true") | ("false", "false") | ("null", "null") => true,
            ("identifier", "identifier") => left_text == right_text,
            _ => false,
        }
    }

    /// From an expect() call node, get left (expect arg) and right (matcher arg) and return true if trivial (same value).
    pub(crate) fn expect_is_trivial(
        tree: &Tree,
        source: &str,
        call_start: usize,
        call_end: usize,
    ) -> bool {
        let root = tree.root_node();
        let call_node = match root.descendant_for_byte_range(call_start, call_end) {
            Some(n) => n,
            None => return false,
        };
        let args = match call_node.child_by_field_name("arguments") {
            Some(a) => a,
            None => return false,
        };
        let mut cursor = args.walk();
        let left = match args.named_children(&mut cursor).next() {
            Some(n) => n,
            None => return false,
        };
        let mut current = call_node;
        loop {
            let parent = match current.parent() {
                Some(p) => p,
                None => return false,
            };
            if parent.kind() == "call_expression" {
                let par_args = match parent.child_by_field_name("arguments") {
                    Some(a) => a,
                    None => return false,
                };
                let mut c2 = par_args.walk();
                let right = match par_args.named_children(&mut c2).next() {
                    Some(n) => n,
                    None => return false,
                };
                return Self::ast_values_equal(left, right, source.as_bytes());
            }
            current = parent;
        }
    }
}

impl Default for TrivialAssertionRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for TrivialAssertionRule {
    fn name(&self) -> &'static str {
        "trivial-assertion"
    }

    fn analyze(&self, tests: &[TestCase], source: &str, tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();
        let lang = TypeScriptParser::language();
        let cache = global_query_cache();
        let bytes = source.as_bytes();

        let mut trivial_locations: Vec<Location> = Vec::new();

        if let Ok(matches) = cache.run_cached_query(source, tree, &lang, QueryId::ExpectCall) {
            for caps in matches {
                let is_expect = caps
                    .iter()
                    .find(|c| c.name == "fn")
                    .map(|c| c.text.as_str())
                    == Some("expect")
                    || caps
                        .iter()
                        .find(|c| c.name == "obj")
                        .map(|c| c.text.as_str())
                        == Some("expect");
                if !is_expect {
                    continue;
                }
                let call_cap = match caps.iter().find(|c| c.name == "call") {
                    Some(c) => c,
                    None => continue,
                };
                if !Self::expect_is_trivial(tree, source, call_cap.start_byte, call_cap.end_byte) {
                    continue;
                }
                let (line, col) = call_cap.start_point;
                let loc = Location::new(line, col);
                trivial_locations.push(loc.clone());
                let raw_preview = source
                    .get(call_cap.start_byte..call_cap.end_byte)
                    .unwrap_or("")
                    .trim();
                let preview = if raw_preview.len() > 50 {
                    format!("{}...", &raw_preview[..47])
                } else {
                    raw_preview.to_string()
                };
                let root = tree.root_node();
                let expect_node =
                    root.descendant_for_byte_range(call_cap.start_byte, call_cap.end_byte);
                let test_name = expect_node
                    .and_then(|n| containing_test_body(n, tree, source))
                    .and_then(|body| {
                        let args = body.parent()?.child_by_field_name("arguments")?;
                        let mut c = args.walk();
                        let name_node = args.named_children(&mut c).next()?;
                        let s = name_node.utf8_text(bytes).ok()?;
                        Some(s.trim_matches(|c| c == '"' || c == '\'').to_string())
                    })
                    .unwrap_or_else(|| "test".to_string());
                issues.push(Issue {
                    rule: Rule::TrivialAssertion,
                    severity: Severity::Warning,
                    message: format!(
                        "Trivial assertion in '{}': always passes and does not verify behavior — {}",
                        test_name, preview
                    ),
                    location: loc,
                    suggestion: Some(
                        "Assert on the actual result of the code under test (e.g. expect(actualResult).toBe(expected)) instead of literals.".to_string(),
                    ),
                    fix: None,
                });
            }
        }

        // Fallback: regex on extracted assertions (when AST didn't cover or for test-level summary)
        for test in tests {
            if test.is_skipped {
                continue;
            }
            let mut trivial_count = 0;
            for assertion in &test.assertions {
                let raw = assertion.raw.trim();
                if raw.is_empty() {
                    continue;
                }
                if trivial_locations.iter().any(|l| {
                    l.line == assertion.location.line && l.column == assertion.location.column
                }) {
                    trivial_count += 1;
                    continue;
                }
                if Self::is_trivial_literal(raw)
                    || Self::same_number_both_sides(raw)
                    || Self::is_trivial_same_number(raw)
                    || Self::is_trivial_same_string(raw)
                    || Self::is_trivial_same_identifier(raw)
                {
                    trivial_count += 1;
                    issues.push(Issue {
                        rule: Rule::TrivialAssertion,
                        severity: Severity::Warning,
                        message: format!(
                            "Trivial assertion in '{}': always passes and does not verify behavior — {}",
                            test.name,
                            if raw.len() > 50 { format!("{}...", &raw[..47]) } else { raw.to_string() }
                        ),
                        location: assertion.location.clone(),
                        suggestion: Some(
                            "Assert on the actual result of the code under test (e.g. expect(actualResult).toBe(expected)) instead of literals.".to_string(),
                        ),
                        fix: None,
                    });
                }
            }
            let total = test.assertions.len();
            if total > 0 && trivial_count == total {
                issues.push(Issue {
                    rule: Rule::TrivialAssertion,
                    severity: Severity::Error,
                    message: format!(
                        "Test '{}' only has trivial assertions — it does not test any real behavior",
                        test.name
                    ),
                    location: test.location.clone(),
                    suggestion: Some(
                        "Replace with assertions on the result of the code under test (e.g. expect(myFunction()).toBe(expected)).".to_string(),
                    ),
                    fix: None,
                });
            }
        }

        issues
    }

    fn calculate_score(&self, tests: &[TestCase], issues: &[Issue]) -> u8 {
        let trivial_count = issues
            .iter()
            .filter(|i| i.rule == Rule::TrivialAssertion)
            .count();
        if tests.is_empty() {
            return 25;
        }
        let deduction = (trivial_count as i32 * 2).min(15);
        (25 - deduction).max(0) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Assertion, AssertionKind, Location};

    fn test_case(name: &str, assertions: Vec<Assertion>) -> TestCase {
        TestCase {
            name: name.to_string(),
            location: Location::new(1, 1),
            is_async: false,
            is_skipped: false,
            assertions,
            describe_block: None,
        }
    }

    fn assertion(kind: AssertionKind, raw: &str) -> Assertion {
        let quality = kind.quality();
        Assertion {
            kind,
            quality,
            location: Location::new(1, 1),
            raw: raw.to_string(),
        }
    }

    #[test]
    fn flags_trivial_literal_assertion() {
        let source = "it('trivial test', () => { expect(1).toBe(1); });";
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse(source)
            .unwrap();
        let parser = crate::parser::TestFileParser::new(source);
        let tests = parser.extract_tests(&tree);
        let rule = TrivialAssertionRule::new();
        let issues = rule.analyze(&tests, source, &tree);
        assert!(!issues.is_empty());
        assert!(issues.iter().any(|i| i.rule == Rule::TrivialAssertion));
    }

    #[test]
    fn flags_true_tobe_true() {
        let source = "it('always passes', () => { expect(true).toBe(true); });";
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse(source)
            .unwrap();
        let parser = crate::parser::TestFileParser::new(source);
        let tests = parser.extract_tests(&tree);
        let rule = TrivialAssertionRule::new();
        let issues = rule.analyze(&tests, source, &tree);
        assert!(!issues.is_empty());
    }

    #[test]
    fn no_issue_for_meaningful_assertion() {
        let source = "it('real test', () => { expect(myFunc()).toBe(42); });";
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse(source)
            .unwrap();
        let parser = crate::parser::TestFileParser::new(source);
        let tests = parser.extract_tests(&tree);
        let rule = TrivialAssertionRule::new();
        let issues = rule.analyze(&tests, source, &tree);
        assert!(issues.is_empty());
    }

    #[test]
    fn flags_trivial_same_number_any_value() {
        let sources = [
            "it('two', () => { expect(2).toBe(2); });",
            "it('forty-two', () => { expect(42).toEqual(42); });",
        ];
        let rule = TrivialAssertionRule::new();
        for source in &sources {
            let tree = crate::parser::TypeScriptParser::new()
                .unwrap()
                .parse(source)
                .unwrap();
            let parser = crate::parser::TestFileParser::new(source);
            let tests = parser.extract_tests(&tree);
            let issues = rule.analyze(&tests, source, &tree);
            assert!(
                !issues.is_empty(),
                "should flag trivial same-number assertion"
            );
        }
    }

    #[test]
    fn flags_trivial_same_string_literal() {
        let source = "it('literal string', () => { expect('hello').toBe('hello'); });";
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse(source)
            .unwrap();
        let parser = crate::parser::TestFileParser::new(source);
        let tests = parser.extract_tests(&tree);
        let rule = TrivialAssertionRule::new();
        let issues = rule.analyze(&tests, source, &tree);
        assert!(!issues.is_empty());
    }

    #[test]
    fn flags_trivial_same_identifier() {
        let source = "it('array identity', () => { expect(arr).toEqual(arr); });";
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse(source)
            .unwrap();
        let parser = crate::parser::TestFileParser::new(source);
        let tests = parser.extract_tests(&tree);
        let rule = TrivialAssertionRule::new();
        let issues = rule.analyze(&tests, source, &tree);
        assert!(!issues.is_empty());
    }

    #[test]
    fn regex_fallback_with_prebuilt_tests() {
        let tests = vec![test_case(
            "trivial test",
            vec![assertion(AssertionKind::ToBe, "expect(1).toBe(1)")],
        )];
        let rule = TrivialAssertionRule::new();
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse("x")
            .unwrap();
        let issues = rule.analyze(&tests, "", &tree);
        assert!(!issues.is_empty());
    }
}
