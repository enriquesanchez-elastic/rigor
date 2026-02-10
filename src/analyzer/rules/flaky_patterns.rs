//! Flaky test pattern detection - non-deterministic code without mocks.
//! Uses tree-sitter queries to avoid false positives in comments and string literals.

use super::AnalysisRule;
use crate::parser::{
    global_query_cache, is_inside_comment_range, is_inside_string_literal_range, QueryId,
    TypeScriptParser,
};
use crate::{Issue, Location, Rule, Severity, TestCase, TestFramework};
use tree_sitter::Tree;

enum TimerSuggestionKind {
    Date,
    Advance,
}

/// Rule for detecting flaky patterns (Date.now, Math.random, timers, etc.)
pub struct FlakyPatternsRule {
    framework: Option<TestFramework>,
}

impl FlakyPatternsRule {
    pub fn new() -> Self {
        Self { framework: None }
    }

    pub fn with_framework(mut self, framework: TestFramework) -> Self {
        self.framework = Some(framework);
        self
    }

    fn timer_suggestion(&self, kind: TimerSuggestionKind) -> String {
        match self.framework {
            Some(TestFramework::Vitest) => match kind {
                TimerSuggestionKind::Date => {
                    "Use vi.useFakeTimers() and vi.setSystemTime() for deterministic dates"
                        .to_string()
                }
                TimerSuggestionKind::Advance => {
                    "Use vi.useFakeTimers() and vi.advanceTimersByTime()".to_string()
                }
            },
            Some(TestFramework::Playwright) => match kind {
                TimerSuggestionKind::Date => {
                    "Use page.clock or test.use(clock) for deterministic time in Playwright"
                        .to_string()
                }
                TimerSuggestionKind::Advance => {
                    "Use page.clock and advance time with clock.tick() in Playwright".to_string()
                }
            },
            Some(TestFramework::Cypress) => match kind {
                TimerSuggestionKind::Date => {
                    "Use cy.clock() and cy.tick() for deterministic dates in Cypress".to_string()
                }
                TimerSuggestionKind::Advance => {
                    "Use cy.clock() and cy.tick() to control timers in Cypress".to_string()
                }
            },
            _ => match kind {
                TimerSuggestionKind::Date => {
                    "Use jest.useFakeTimers() and jest.setSystemTime() for deterministic dates"
                        .to_string()
                }
                TimerSuggestionKind::Advance => {
                    "Use jest.useFakeTimers() and jest.advanceTimersByTime()".to_string()
                }
            },
        }
    }

    fn random_mock_suggestion(&self) -> String {
        match self.framework {
            Some(TestFramework::Vitest) => {
                "Use vi.spyOn(Math, 'random').mockReturnValue(0.5) or similar".to_string()
            }
            Some(TestFramework::Playwright) | Some(TestFramework::Cypress) => {
                "Mock Math.random for deterministic results in this test".to_string()
            }
            _ => "Use jest.spyOn(Math, 'random').mockReturnValue(0.5) or similar".to_string(),
        }
    }

    /// Check if file has fake timers (jest.useFakeTimers, vi.useFakeTimers) via source.
    fn has_fake_timers(source: &str) -> bool {
        source.contains("useFakeTimers")
    }

    /// Check if Math.random is mocked
    fn has_random_mock(source: &str) -> bool {
        source.contains("spyOn(Math")
            || source.contains("spyOn(globalThis, 'Math')")
            || source.contains("mockReturnValue")
                && (source.contains("random") || source.contains("Math"))
    }

    /// Check if fetch/network is mocked
    fn has_fetch_mock(source: &str) -> bool {
        source.contains("jest.mock(") && (source.contains("fetch") || source.contains("axios"))
            || source.contains("vi.mock(") && (source.contains("fetch") || source.contains("axios"))
            || source.contains("mockImplementation") && source.contains("fetch")
    }

    fn push_issue(
        issues: &mut Vec<Issue>,
        line: usize,
        col: usize,
        message: &str,
        suggestion: String,
    ) {
        issues.push(Issue {
            rule: Rule::FlakyPattern,
            severity: if message.contains("new Date()") {
                Severity::Info
            } else {
                Severity::Warning
            },
            message: message.to_string(),
            location: Location::new(line, col),
            suggestion: Some(suggestion),
            fix: None,
        });
    }
}

impl Default for FlakyPatternsRule {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisRule for FlakyPatternsRule {
    fn name(&self) -> &'static str {
        "flaky-patterns"
    }

    fn analyze(&self, _tests: &[TestCase], source: &str, tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();
        let has_fake_timers = Self::has_fake_timers(source);
        let has_random_mock = Self::has_random_mock(source);
        let has_fetch_mock = Self::has_fetch_mock(source);
        let root = tree.root_node();
        let lang = TypeScriptParser::language();
        let cache = global_query_cache();

        // Date.now() and new Date() via AST
        if let Ok(date_matches) = cache.run_cached_query(source, tree, &lang, QueryId::DateNow) {
            for caps in date_matches {
                let (line, col) = caps.first().map(|c| c.start_point).unwrap_or((1, 1));
                let start_byte = caps.first().map(|c| c.start_byte).unwrap_or(0);
                let end_byte = caps.first().map(|c| c.end_byte).unwrap_or(0);
                if is_inside_comment_range(start_byte, end_byte, source) {
                    continue;
                }
                if is_inside_string_literal_range(start_byte, end_byte, root) {
                    continue;
                }
                let obj = caps
                    .iter()
                    .find(|c| c.name == "obj")
                    .map(|c| c.text.as_str());
                let prop = caps
                    .iter()
                    .find(|c| c.name == "prop")
                    .map(|c| c.text.as_str());
                let ctor = caps
                    .iter()
                    .find(|c| c.name == "ctor")
                    .map(|c| c.text.as_str());
                if matches!((obj, prop), (Some("Date"), Some("now"))) && !has_fake_timers {
                    Self::push_issue(
                        &mut issues,
                        line,
                        col,
                        "Date.now() is non-deterministic - use fake timers or mock it",
                        self.timer_suggestion(TimerSuggestionKind::Date),
                    );
                } else if ctor == Some("Date") && !has_fake_timers {
                    Self::push_issue(
                        &mut issues,
                        line,
                        col,
                        "new Date() is non-deterministic - consider using fake timers",
                        self.timer_suggestion(TimerSuggestionKind::Date),
                    );
                }
            }
        }

        // Math.random() via AST
        if let Ok(math_matches) = cache.run_cached_query(source, tree, &lang, QueryId::MathRandom) {
            for caps in math_matches {
                let obj = caps
                    .iter()
                    .find(|c| c.name == "obj")
                    .map(|c| c.text.as_str());
                let prop = caps
                    .iter()
                    .find(|c| c.name == "prop")
                    .map(|c| c.text.as_str());
                if obj != Some("Math") || prop != Some("random") {
                    continue;
                }
                let (line, col) = caps.first().map(|c| c.start_point).unwrap_or((1, 1));
                let start_byte = caps.first().map(|c| c.start_byte).unwrap_or(0);
                let end_byte = caps.first().map(|c| c.end_byte).unwrap_or(0);
                if is_inside_comment_range(start_byte, end_byte, source)
                    || is_inside_string_literal_range(start_byte, end_byte, root)
                {
                    continue;
                }
                if !has_random_mock {
                    Self::push_issue(
                        &mut issues,
                        line,
                        col,
                        "Math.random() is non-deterministic - mock it for reproducible tests",
                        self.random_mock_suggestion(),
                    );
                }
            }
        }

        // setTimeout / setInterval via AST
        if let Ok(timer_matches) =
            cache.run_cached_query(source, tree, &lang, QueryId::SetTimeoutInterval)
        {
            for caps in timer_matches {
                let fn_name = caps
                    .iter()
                    .find(|c| c.name == "fn")
                    .map(|c| c.text.as_str());
                if fn_name != Some("setTimeout") && fn_name != Some("setInterval") {
                    continue;
                }
                let (line, col) = caps.first().map(|c| c.start_point).unwrap_or((1, 1));
                let start_byte = caps.first().map(|c| c.start_byte).unwrap_or(0);
                let end_byte = caps.first().map(|c| c.end_byte).unwrap_or(0);
                if is_inside_comment_range(start_byte, end_byte, source)
                    || is_inside_string_literal_range(start_byte, end_byte, root)
                {
                    continue;
                }
                if !has_fake_timers {
                    // Check for numeric delay on the same line (e.g. setTimeout(fn, 1000))
                    let line_src = source.lines().nth(line.saturating_sub(1)).unwrap_or("");
                    let has_numeric_delay = line_src
                        .find("setTimeout(")
                        .or_else(|| line_src.find("setInterval("))
                        .and_then(|start| {
                            let args = line_src.get(start..).unwrap_or("");
                            let comma_pos = args.find(',')?;
                            let after = args.get(comma_pos + 1..).unwrap_or("").trim_start();
                            Some(after.starts_with(|c: char| c.is_ascii_digit()))
                        })
                        == Some(true);
                    if has_numeric_delay {
                        Self::push_issue(
                            &mut issues,
                            line,
                            col,
                            "setTimeout/setInterval with literal delay can cause flaky tests",
                            self.timer_suggestion(TimerSuggestionKind::Advance),
                        );
                    }
                }
            }
        }

        // fetch / axios via AST
        if let Ok(fetch_matches) =
            cache.run_cached_query(source, tree, &lang, QueryId::FetchAxiosCall)
        {
            for caps in fetch_matches {
                let fn_name = caps
                    .iter()
                    .find(|c| c.name == "fn")
                    .map(|c| c.text.as_str());
                let obj = caps
                    .iter()
                    .find(|c| c.name == "obj")
                    .map(|c| c.text.as_str());
                let prop = caps
                    .iter()
                    .find(|c| c.name == "prop")
                    .map(|c| c.text.as_str());
                let is_fetch = fn_name == Some("fetch");
                let is_axios = fn_name == Some("axios")
                    || (obj == Some("axios")
                        && prop
                            .map(|p| {
                                p.starts_with("get") || p.starts_with("post") || p == "request"
                            })
                            .unwrap_or(false));
                if !is_fetch && !is_axios {
                    continue;
                }
                let (line, col) = caps.first().map(|c| c.start_point).unwrap_or((1, 1));
                let start_byte = caps.first().map(|c| c.start_byte).unwrap_or(0);
                let end_byte = caps.first().map(|c| c.end_byte).unwrap_or(0);
                if is_inside_comment_range(start_byte, end_byte, source)
                    || is_inside_string_literal_range(start_byte, end_byte, root)
                {
                    continue;
                }
                if !has_fetch_mock {
                    Self::push_issue(
                        &mut issues,
                        line,
                        col,
                        "Network call (fetch/axios) without mock - test may be flaky or slow",
                        "Mock fetch/axios with jest.mock() or MSW for unit tests".to_string(),
                    );
                }
            }
        }

        // Fallback: line-based if queries failed (e.g. parse error or unsupported grammar)
        if issues.is_empty()
            && (source.contains("Date.now()")
                || source.contains("Math.random()")
                || source.contains("setTimeout(")
                || source.contains("fetch("))
        {
            let has_any_ast = cache
                .run_cached_query(source, tree, &lang, QueryId::DateNow)
                .is_ok();
            if !has_any_ast {
                for (zero_indexed, line) in source.lines().enumerate() {
                    let line_no = zero_indexed + 1;
                    let trimmed = line.trim();
                    if trimmed.starts_with("//") || trimmed.starts_with("/*") {
                        continue;
                    }
                    if trimmed.contains("Date.now()") && !has_fake_timers {
                        let col = line.find("Date.now()").unwrap_or(0) + 1;
                        Self::push_issue(
                            &mut issues,
                            line_no,
                            col,
                            "Date.now() is non-deterministic - use fake timers or mock it",
                            self.timer_suggestion(TimerSuggestionKind::Date),
                        );
                    }
                    if (trimmed.contains("new Date()") || trimmed.contains("new Date ("))
                        && !has_fake_timers
                    {
                        let col = line.find("new Date").unwrap_or(0) + 1;
                        Self::push_issue(
                            &mut issues,
                            line_no,
                            col,
                            "new Date() is non-deterministic - consider using fake timers",
                            self.timer_suggestion(TimerSuggestionKind::Date),
                        );
                    }
                    if trimmed.contains("Math.random()") && !has_random_mock {
                        let col = line.find("Math.random()").unwrap_or(0) + 1;
                        Self::push_issue(
                            &mut issues,
                            line_no,
                            col,
                            "Math.random() is non-deterministic - mock it for reproducible tests",
                            self.random_mock_suggestion(),
                        );
                    }
                    if (trimmed.contains("fetch(")
                        || trimmed.contains("axios.")
                        || trimmed.contains("axios("))
                        && !trimmed.contains("mock")
                        && !has_fetch_mock
                    {
                        let col = line
                            .find("fetch(")
                            .or_else(|| line.find("axios"))
                            .unwrap_or(0)
                            + 1;
                        Self::push_issue(
                            &mut issues,
                            line_no,
                            col,
                            "Network call (fetch/axios) without mock - test may be flaky or slow",
                            "Mock fetch/axios with jest.mock() or MSW for unit tests".to_string(),
                        );
                    }
                }
            }
        }

        issues
    }

    fn calculate_score(&self, _tests: &[TestCase], issues: &[Issue]) -> u8 {
        let count = issues
            .iter()
            .filter(|i| i.rule == Rule::FlakyPattern)
            .count();
        let mut score: i32 = 25;
        score -= (count as i32 * 4).min(20);
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
    fn positive_detects_date_now_without_fake_timers() {
        let rule = FlakyPatternsRule::new();
        let source = r#"
        it('uses time', () => {
            const t = Date.now();
            expect(t).toBeGreaterThan(0);
        });
        "#;
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse(source)
            .unwrap();
        let issues = rule.analyze(&make_empty_tests(), source, &tree);
        assert!(
            !issues.is_empty(),
            "should detect Date.now() without useFakeTimers"
        );
        assert!(issues.iter().any(|i| i.rule == Rule::FlakyPattern));
    }

    #[test]
    fn positive_detects_math_random_without_mock() {
        let rule = FlakyPatternsRule::new();
        let source = "const x = Math.random();";
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse(source)
            .unwrap();
        let issues = rule.analyze(&make_empty_tests(), source, &tree);
        assert!(!issues.is_empty());
        assert!(issues.iter().any(|i| i.rule == Rule::FlakyPattern));
    }

    #[test]
    fn negative_no_issues_with_fake_timers() {
        let rule = FlakyPatternsRule::new();
        let source = r#"
        beforeEach(() => { jest.useFakeTimers(); });
        it('uses time', () => {
            const t = Date.now();
            expect(t).toBeGreaterThan(0);
        });
        "#;
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse(source)
            .unwrap();
        let issues = rule.analyze(&make_empty_tests(), source, &tree);
        assert!(
            issues.is_empty(),
            "useFakeTimers should suppress Date.now() issue"
        );
    }

    #[test]
    fn negative_clean_source_no_issues() {
        let rule = FlakyPatternsRule::new();
        let source = "it('adds numbers', () => { expect(1 + 1).toBe(2); });";
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse(source)
            .unwrap();
        let issues = rule.analyze(&make_empty_tests(), source, &tree);
        assert!(issues.is_empty());
    }

    #[test]
    fn negative_date_now_in_comment_no_issue() {
        let rule = FlakyPatternsRule::new();
        let source = r#"
        it('test', () => {
            // Date.now() is used elsewhere
            expect(1).toBe(1);
        });
        "#;
        let tree = crate::parser::TypeScriptParser::new()
            .unwrap()
            .parse(source)
            .unwrap();
        let issues = rule.analyze(&make_empty_tests(), source, &tree);
        assert!(
            !issues.iter().any(|i| i.message.contains("Date.now()")),
            "should not flag Date.now() inside comment"
        );
    }

    #[test]
    fn score_decreases_with_more_issues() {
        let rule = FlakyPatternsRule::new();
        let tests = make_empty_tests();
        let zero_issues: Vec<Issue> = vec![];
        let one_issue = vec![Issue {
            rule: Rule::FlakyPattern,
            severity: Severity::Warning,
            message: "test".to_string(),
            location: Location::new(1, 1),
            suggestion: None,
            fix: None,
        }];
        assert_eq!(rule.calculate_score(&tests, &zero_issues), 25);
        assert_eq!(rule.calculate_score(&tests, &one_issue), 21);
    }
}
