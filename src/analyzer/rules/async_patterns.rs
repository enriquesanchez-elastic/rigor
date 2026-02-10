//! Async test patterns - missing await on promises.
//! Uses tree-sitter to find expect().resolves/.rejects not under await.

use super::AnalysisRule;
use crate::parser::{global_query_cache, QueryId, TypeScriptParser};
use crate::{Issue, Location, Rule, Severity, TestCase};
use tree_sitter::Tree;

/// Rule for detecting missing await in async tests
pub struct AsyncPatternsRule;

impl AsyncPatternsRule {
    pub fn new() -> Self {
        Self
    }

    /// Check if this expect() call is part of .resolves/.rejects chain and not under await.
    fn expect_resolves_rejects_without_await(
        tree: &Tree,
        source: &str,
        call_start: usize,
        call_end: usize,
    ) -> Option<(usize, usize, &'static str)> {
        let root = tree.root_node();
        let call_node = root.descendant_for_byte_range(call_start, call_end)?;
        let mut kind: Option<&'static str> = None;
        let mut current = call_node;
        while let Some(parent) = current.parent() {
            if parent.kind() == "await_expression" {
                return None;
            }
            if parent.kind() == "member_expression" {
                if let Some(prop) = parent.child_by_field_name("property") {
                    let text = prop.utf8_text(source.as_bytes()).ok()?;
                    if text == "resolves" {
                        kind = Some("resolves");
                    } else if text == "rejects" {
                        kind = Some("rejects");
                    }
                }
            }
            current = parent;
        }
        let k = kind?;
        let (line, col) = (
            call_node.start_position().row + 1,
            call_node.start_position().column + 1,
        );
        Some((line, col, k))
    }
}

impl Default for AsyncPatternsRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for AsyncPatternsRule {
    fn name(&self) -> &'static str {
        "async-patterns"
    }

    fn analyze(&self, tests: &[TestCase], source: &str, tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();
        let lang = TypeScriptParser::language();
        let cache = global_query_cache();

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
                if let Some((line, col, kind)) = Self::expect_resolves_rejects_without_await(
                    tree,
                    source,
                    call_cap.start_byte,
                    call_cap.end_byte,
                ) {
                    issues.push(Issue {
                        rule: Rule::MissingAwait,
                        severity: Severity::Warning,
                        message: format!(
                            "expect().{} used without await - use 'await expect(...).{}' in async tests",
                            kind, kind
                        ),
                        location: Location::new(line, col),
                        suggestion: Some(
                            "Prefer: await expect(asyncFn()).resolves.toBe(value) or await expect(promise).rejects.toThrow()".to_string(),
                        ),
                        fix: None,
                    });
                }
            }
        }

        // Async tests with no await in body (keep line-based scan for test body)
        for test in tests {
            if !test.is_async || test.is_skipped {
                continue;
            }
            let start = test.location.line.saturating_sub(1);
            let end_line = test.location.end_line.unwrap_or(test.location.line);
            let line_count = end_line.saturating_sub(test.location.line) + 1;
            let test_lines: Vec<&str> = source.lines().skip(start).take(line_count).collect();
            let has_await = test_lines.iter().any(|l| {
                let t = l.trim();
                !t.starts_with("//") && t.contains("await ")
            });
            if !has_await && test_lines.len() > 1 {
                let has_returns_promise = test_lines.iter().any(|l| {
                    l.contains("return ") && (l.contains("expect(") || l.contains("Promise"))
                });
                if !has_returns_promise {
                    issues.push(Issue {
                        rule: Rule::MissingAwait,
                        severity: Severity::Info,
                        message: format!(
                            "Async test '{}' may be missing await on async calls",
                            test.name
                        ),
                        location: test.location.clone(),
                        suggestion: Some(
                            "Ensure async operations are awaited to avoid race conditions"
                                .to_string(),
                        ),
                        fix: None,
                    });
                }
            }
        }

        issues
    }

    fn calculate_score(&self, _tests: &[TestCase], issues: &[Issue]) -> u8 {
        let count = issues
            .iter()
            .filter(|i| i.rule == Rule::MissingAwait)
            .count();
        let mut score: i32 = 25;
        score -= (count as i32 * 3).min(15);
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
    fn positive_detects_resolves_without_await() {
        let rule = AsyncPatternsRule::new();
        let source = "expect(promise).resolves.toBe(5);";
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse(source)
            .unwrap();
        let issues = rule.analyze(&make_empty_tests(), source, &tree);
        assert!(!issues.is_empty());
        assert!(issues.iter().any(|i| i.rule == Rule::MissingAwait));
    }

    #[test]
    fn negative_await_expect_no_issue() {
        let rule = AsyncPatternsRule::new();
        let source = "await expect(asyncFn()).resolves.toBe(42);";
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse(source)
            .unwrap();
        let issues = rule.analyze(&make_empty_tests(), source, &tree);
        assert!(issues.is_empty());
    }

    #[test]
    fn score_decreases_with_issues() {
        let rule = AsyncPatternsRule::new();
        let tests = make_empty_tests();
        let zero_issues: Vec<Issue> = vec![];
        let one_issue = vec![Issue {
            rule: Rule::MissingAwait,
            severity: Severity::Warning,
            message: "test".to_string(),
            location: Location::new(1, 1),
            suggestion: None,
            fix: None,
        }];
        assert_eq!(rule.calculate_score(&tests, &zero_issues), 25);
        assert_eq!(rule.calculate_score(&tests, &one_issue), 22);
    }
}
