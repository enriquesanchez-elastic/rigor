//! Test relevance analysis: are tests *meaningful*?
//!
//! **Relevance** here means: does the test actually test what it claims to test,
//! and does it assert on the right behavior? A test can be "valid" (runs, has
//! assertions) but still irrelevant — e.g. name says "returns 404" but only
//! checks `expect(res).toBeDefined()`, or assertions that don't depend on the
//! mutated behavior.
//!
//! Two layers:
//!
//! 1. **Mutation-driven relevance** (this module): Survived mutants show where
//!    tests did not catch the change — i.e. tests are not *relevant* to those
//!    behaviors. We turn this into actionable feedback (which lines, what kind
//!    of assertion would help).
//!
//! 2. **Static relevance** (analyzer rules): Assertion–intent mismatch (name
//!    implies "returns X" / "throws" but no matching assertion), trivial
//!    assertions (expect(1).toBe(1)), etc. These catch "tests that make no
//!    sense" without running mutation.

use std::collections::HashMap;

use super::{MutationResult, MutationRun};

/// Summary of test relevance: which source locations had no test coverage (survived mutants).
#[derive(Debug, Clone)]
pub struct RelevanceSummary {
    /// Source path (same as mutation result).
    pub source_path: std::path::PathBuf,
    /// Kill rate 0–100 (same as mutation score).
    pub kill_rate_percent: u32,
    /// Number of distinct source lines that had at least one survived mutant.
    pub lines_with_survived: usize,
    /// Survived mutants grouped by source line (line -> list of mutation descriptions).
    pub survived_by_line: HashMap<usize, Vec<SurvivedAtLine>>,
    /// Human-oriented suggestions for improving test relevance.
    pub suggestions: Vec<String>,
}

/// A survived mutant at a specific line (for grouping).
#[derive(Debug, Clone)]
pub struct SurvivedAtLine {
    pub line: usize,
    pub column: usize,
    pub description: String,
    pub original: String,
    pub replacement: String,
}

/// Map mutation description to a short relevance hint (what kind of test would catch this).
fn suggestion_hint(description: &str) -> &'static str {
    if description.contains("return") {
        "Assert on the exact return value (e.g. expect(result).toEqual(...)) so null/undefined mutants are caught."
    } else if description.contains(">=")
        || description.contains("<=")
        || description.contains("> to")
        || description.contains("< to")
    {
        "Add boundary tests (e.g. for x >= 18 test 17, 18, 19) so comparison mutants are caught."
    } else if description.contains("array") || description.contains("index") {
        "Assert on array length or element at index so array/index mutants are caught."
    } else if description.contains("string") || description.contains("empty") {
        "Assert on string content or length so empty-string mutants are caught."
    } else if description.contains("++")
        || description.contains("--")
        || description.contains("+=")
        || description.contains("-=")
    {
        "Assert on the final value or side effect after the operation so increment/decrement mutants are caught."
    } else if description.contains("true") || description.contains("false") {
        "Assert on the exact boolean or outcome so true/false swap mutants are caught."
    } else if description.contains("===") || description.contains("!= ") {
        "Assert on equality so comparison mutants are caught."
    } else {
        "Add or strengthen assertions that depend on this behavior."
    }
}

/// Build a relevance summary from mutation testing results.
pub fn relevance_summary(result: &MutationResult) -> RelevanceSummary {
    let kill_rate_percent = if result.total > 0 {
        (result.killed as f32 / result.total as f32 * 100.0) as u32
    } else {
        0
    };

    let survived: Vec<&MutationRun> = result.details.iter().filter(|r| !r.killed).collect();
    let mut survived_by_line: HashMap<usize, Vec<SurvivedAtLine>> = HashMap::new();
    let mut seen_hints = std::collections::HashSet::new();
    let mut suggestions = Vec::new();

    for run in &survived {
        let line = run.mutation.line;
        let entry = SurvivedAtLine {
            line: run.mutation.line,
            column: run.mutation.column,
            description: run.mutation.description.clone(),
            original: run.mutation.original.clone(),
            replacement: run.mutation.replacement.clone(),
        };
        survived_by_line.entry(line).or_default().push(entry);

        let hint = suggestion_hint(&run.mutation.description);
        if seen_hints.insert(hint) {
            suggestions.push(hint.to_string());
        }
    }

    let lines_with_survived = survived_by_line.len();

    RelevanceSummary {
        source_path: result.source_path.clone(),
        kill_rate_percent,
        lines_with_survived,
        survived_by_line,
        suggestions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mutation::Mutation;

    #[test]
    fn test_relevance_summary_from_result() {
        let result = MutationResult {
            source_path: std::path::PathBuf::from("src/foo.ts"),
            total: 3,
            killed: 1,
            survived: 2,
            details: vec![
                MutationRun {
                    mutation: Mutation {
                        start: 0,
                        end: 2,
                        line: 10,
                        column: 1,
                        original: ">=".to_string(),
                        replacement: ">".to_string(),
                        description: ">= to >".to_string(),
                    },
                    killed: true,
                    stdout: String::new(),
                    stderr: String::new(),
                },
                MutationRun {
                    mutation: Mutation {
                        start: 5,
                        end: 10,
                        line: 12,
                        column: 1,
                        original: "return 42;".to_string(),
                        replacement: "return null;".to_string(),
                        description: "return to null/undefined".to_string(),
                    },
                    killed: false,
                    stdout: String::new(),
                    stderr: String::new(),
                },
                MutationRun {
                    mutation: Mutation {
                        start: 11,
                        end: 13,
                        line: 12,
                        column: 2,
                        original: "return 0;".to_string(),
                        replacement: "return undefined;".to_string(),
                        description: "return to null/undefined".to_string(),
                    },
                    killed: false,
                    stdout: String::new(),
                    stderr: String::new(),
                },
            ],
        };

        let summary = relevance_summary(&result);
        assert_eq!(summary.kill_rate_percent, 33); // 1/3 killed
        assert_eq!(summary.lines_with_survived, 1); // only line 12
        assert_eq!(
            summary
                .survived_by_line
                .get(&12)
                .map(|v| v.len())
                .unwrap_or(0),
            2
        );
        assert!(!summary.suggestions.is_empty());
    }
}
