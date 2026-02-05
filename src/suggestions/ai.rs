//! AI suggestion generator for test improvements

use crate::mutation::MutationResult;
use crate::{AnalysisResult, Issue, Rule, Severity};
use std::fs;
use std::path::Path;

/// Generator for AI-powered improvement suggestions
pub struct AiSuggestionGenerator {
    /// Include detailed code examples in suggestions
    detailed: bool,
    /// Optional mutation testing results
    mutation_result: Option<MutationResult>,
}

impl AiSuggestionGenerator {
    /// Create a new AI suggestion generator
    pub fn new() -> Self {
        Self {
            detailed: true,
            mutation_result: None,
        }
    }

    /// Set detailed mode
    pub fn detailed(mut self, detailed: bool) -> Self {
        self.detailed = detailed;
        self
    }

    /// Add mutation testing results to enhance the prompt
    pub fn with_mutation_result(mut self, result: MutationResult) -> Self {
        self.mutation_result = Some(result);
        self
    }

    /// Generate a prompt for AI to improve tests
    pub fn generate_prompt(&self, result: &AnalysisResult) -> String {
        let test_content = fs::read_to_string(&result.file_path)
            .unwrap_or_else(|_| "<could not read file>".to_string());

        let source_content = result
            .source_file
            .as_ref()
            .and_then(|p| fs::read_to_string(p).ok())
            .unwrap_or_default();

        let issues_text = self.format_issues_detailed(&result.issues);
        let test_hints = self.generate_test_hints(result, &source_content);
        let score_breakdown = self.format_score_breakdown(result);
        let mutation_section = self.format_mutation_results();

        let prompt = format!(
            r#"You are an expert test quality engineer. Improve the following test file to achieve a score of 90+.

## Analysis Summary
**File:** `{}`
**Framework:** {}
**Current Score:** {}/100 (Grade: {})
{}

## Issues to Fix
{}
{}{}
## Current Test Code
```typescript
{}
```
{}
## Improvement Requirements

### Priority 1: Fix Critical Issues
{}
{}
### Priority 2: Strengthen Assertions
- Replace `.toBeDefined()` with `.toBe(expectedValue)` or `.toEqual(expectedObject)`
- Replace `.toBeTruthy()` with `.toBe(true)` when checking booleans
- Use `.toHaveBeenCalledWith(specificArgs)` instead of just `.toHaveBeenCalled()`

### Priority 3: Add Missing Tests
{}

### Output Format
Provide ONLY the improved test file code. No explanations, no markdown outside the code block.
The code should be complete and runnable.

```typescript
"#,
            result.file_path.display(),
            result.framework,
            result.score.value,
            result.score.grade,
            score_breakdown,
            issues_text,
            if test_hints.is_empty() {
                String::new()
            } else {
                format!("## Test Generation Hints\n{}\n", test_hints)
            },
            mutation_section,
            test_content,
            if source_content.is_empty() {
                String::new()
            } else {
                format!(
                    r#"
## Source File Under Test
```typescript
{}
```
"#,
                    source_content
                )
            },
            self.format_critical_fixes(&result.issues),
            self.format_mutation_fixes(),
            self.format_missing_tests(&result.issues),
        );

        prompt
    }

    /// Format mutation testing results for inclusion in the prompt
    fn format_mutation_results(&self) -> String {
        let Some(ref mutation) = self.mutation_result else {
            return String::new();
        };

        if mutation.total == 0 {
            return String::new();
        }

        let score = mutation.score() as u32;
        let mut output = format!(
            r#"
## Mutation Testing Results
**Mutation Score:** {}% ({}/{} mutants killed)

"#,
            score, mutation.killed, mutation.total
        );

        // List survived mutants (these are opportunities for better tests)
        let survivors: Vec<_> = mutation.details.iter().filter(|r| !r.killed).collect();

        if !survivors.is_empty() {
            output.push_str("### Survived Mutants (Tests didn't catch these changes)\n");
            for (i, run) in survivors.iter().take(10).enumerate() {
                output.push_str(&format!(
                    "{}. Line {}: `{}` → `{}` ({})\n",
                    i + 1,
                    run.mutation.line,
                    run.mutation.original.trim(),
                    run.mutation.replacement.trim(),
                    run.mutation.description
                ));
            }
            if survivors.len() > 10 {
                output.push_str(&format!("   ... and {} more\n", survivors.len() - 10));
            }
            output.push('\n');
        }

        output
    }

    /// Format mutation-specific fix suggestions
    fn format_mutation_fixes(&self) -> String {
        let Some(ref mutation) = self.mutation_result else {
            return String::new();
        };

        let survivors: Vec<_> = mutation.details.iter().filter(|r| !r.killed).collect();

        if survivors.is_empty() {
            return String::new();
        }

        let mut output = String::from("\n### Priority 0: Kill Survived Mutants\n");
        output.push_str("The following source code changes were NOT detected by tests. Add assertions that would fail if these mutations were applied:\n\n");

        for run in survivors.iter().take(5) {
            let suggestion = Self::suggest_fix_for_mutation(run);
            output.push_str(&format!(
                "- **Line {}:** `{}` → `{}`\n  {}\n",
                run.mutation.line,
                run.mutation.original.trim(),
                run.mutation.replacement.trim(),
                suggestion
            ));
        }

        output.push('\n');
        output
    }

    /// Suggest a fix for a specific survived mutation
    fn suggest_fix_for_mutation(run: &crate::mutation::MutationRun) -> String {
        let desc = run.mutation.description.to_lowercase();

        if desc.contains(">=")
            || desc.contains("<=")
            || desc.contains("> to")
            || desc.contains("< to")
        {
            return "→ Add boundary value test: test with the exact boundary value (e.g., if `x >= 5`, test with `x = 5` and `x = 4`)".to_string();
        }

        if desc.contains("true") || desc.contains("false") {
            return "→ Add boolean assertion: verify the exact boolean value, not just truthiness"
                .to_string();
        }

        if desc.contains("+") || desc.contains("-") || desc.contains("*") || desc.contains("/") {
            return "→ Add arithmetic verification: test with specific input values that would produce different results with the mutation".to_string();
        }

        if desc.contains("string") || desc.contains("empty") {
            return "→ Add string content assertion: verify the exact string value, not just that it exists".to_string();
        }

        if desc.contains("return") {
            return "→ Add return value assertion: verify the specific return value, not just that it's defined".to_string();
        }

        if desc.contains("optional chaining") || desc.contains("?.") {
            return "→ Add null safety test: verify behavior when the value is null/undefined"
                .to_string();
        }

        if desc.contains("nullish") || desc.contains("??") {
            return "→ Test nullish coalescing: verify correct handling of null vs falsy values (0, '', false)".to_string();
        }

        "→ Add specific assertion that would fail if this mutation was applied".to_string()
    }

    /// Generate a more concise prompt for quick fixes
    pub fn generate_quick_fix_prompt(&self, result: &AnalysisResult, issue: &Issue) -> String {
        let test_content = fs::read_to_string(&result.file_path)
            .unwrap_or_else(|_| "<could not read file>".to_string());

        let context_lines = self.extract_context(&test_content, issue.location.line, 5);

        format!(
            r#"Fix this test issue:

**Issue:** {} (line {})
**Message:** {}
**Suggestion:** {}

**Context:**
```typescript
{}
```

Provide ONLY the fixed code snippet (the lines that need to change). No explanations.

```typescript
"#,
            issue.rule,
            issue.location.line,
            issue.message,
            issue
                .suggestion
                .as_deref()
                .unwrap_or("See issue description"),
            context_lines,
        )
    }

    /// Generate test hints based on source code analysis
    fn generate_test_hints(&self, result: &AnalysisResult, source_content: &str) -> String {
        if source_content.is_empty() {
            return String::new();
        }

        let mut hints: Vec<String> = Vec::new();

        // Detect functions that throw errors
        if source_content.contains("throw ") || source_content.contains("throw new") {
            let has_error_test = result
                .issues
                .iter()
                .all(|i| i.rule != Rule::MissingErrorTest);
            if !has_error_test {
                hints.push("- **Error paths detected**: Add tests using `expect(() => fn()).toThrow(ErrorType)`".to_string());
            }
        }

        // Detect async functions
        if source_content.contains("async ") || source_content.contains("Promise") {
            hints.push("- **Async code detected**: Ensure tests use `async/await` and test both resolve and reject paths".to_string());
        }

        // Detect conditional logic
        let conditionals =
            source_content.matches(" if ").count() + source_content.matches(" if(").count();
        if conditionals > 2 {
            hints.push(format!(
                "- **{} conditionals detected**: Add tests for each branch (true/false paths)",
                conditionals
            ));
        }

        // Detect numeric comparisons (boundary conditions)
        if source_content.contains(">=")
            || source_content.contains("<=")
            || source_content.contains(" > ")
            || source_content.contains(" < ")
        {
            hints.push("- **Numeric comparisons detected**: Add boundary value tests (value-1, value, value+1)".to_string());
        }

        // Detect array/collection operations
        if source_content.contains(".map(")
            || source_content.contains(".filter(")
            || source_content.contains(".reduce(")
            || source_content.contains(".forEach(")
        {
            hints.push("- **Array operations detected**: Test with empty arrays, single item, and multiple items".to_string());
        }

        // Detect null/undefined checks
        if source_content.contains("=== null")
            || source_content.contains("=== undefined")
            || source_content.contains("!= null")
            || source_content.contains("!= undefined")
        {
            hints.push(
                "- **Null checks detected**: Add tests with null/undefined inputs".to_string(),
            );
        }

        // Detect regex patterns
        if source_content.contains("RegExp")
            || source_content.contains(".match(")
            || source_content.contains(".test(")
        {
            hints.push(
                "- **Regex detected**: Test with matching, non-matching, and edge case inputs"
                    .to_string(),
            );
        }

        // Detect external API calls
        if source_content.contains("fetch(")
            || source_content.contains("axios")
            || source_content.contains("http.")
        {
            hints.push("- **External API calls detected**: Mock these calls and test success/failure scenarios".to_string());
        }

        hints.join("\n")
    }

    /// Generate a prompt and save it to a file
    pub fn generate_prompt_file(
        &self,
        result: &AnalysisResult,
        output_path: &Path,
    ) -> std::io::Result<()> {
        let prompt = self.generate_prompt(result);
        fs::write(output_path, prompt)
    }

    fn format_issues_detailed(&self, issues: &[Issue]) -> String {
        if issues.is_empty() {
            return "No issues found - the test file is in good shape!\n".to_string();
        }

        let mut output = String::new();

        // Group by severity
        let errors: Vec<_> = issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .collect();
        let warnings: Vec<_> = issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
            .collect();
        let infos: Vec<_> = issues
            .iter()
            .filter(|i| i.severity == Severity::Info)
            .collect();

        if !errors.is_empty() {
            output.push_str("### ❌ Errors (Must Fix)\n");
            for issue in errors {
                output.push_str(&format!(
                    "- **Line {}:** `{}` - {}\n",
                    issue.location.line, issue.rule, issue.message
                ));
                if let Some(ref suggestion) = issue.suggestion {
                    output.push_str(&format!("  - Fix: {}\n", suggestion));
                }
            }
            output.push('\n');
        }

        if !warnings.is_empty() {
            output.push_str("### ⚠️ Warnings (Should Fix)\n");
            for issue in warnings {
                output.push_str(&format!(
                    "- **Line {}:** `{}` - {}\n",
                    issue.location.line, issue.rule, issue.message
                ));
                if self.detailed {
                    if let Some(ref suggestion) = issue.suggestion {
                        output.push_str(&format!("  - Fix: {}\n", suggestion));
                    }
                }
            }
            output.push('\n');
        }

        if !infos.is_empty() {
            output.push_str("### 💡 Suggestions (Nice to Have)\n");
            for issue in &infos[..infos.len().min(5)] {
                output.push_str(&format!(
                    "- **Line {}:** `{}` - {}\n",
                    issue.location.line, issue.rule, issue.message
                ));
            }
            if infos.len() > 5 {
                output.push_str(&format!("- ... and {} more suggestions\n", infos.len() - 5));
            }
            output.push('\n');
        }

        output
    }

    fn format_score_breakdown(&self, result: &AnalysisResult) -> String {
        format!(
            r#"
| Category | Score | Status |
|----------|-------|--------|
| Assertion Quality | {}/25 | {} |
| Error Coverage | {}/25 | {} |
| Boundary Conditions | {}/25 | {} |
| Test Isolation | {}/25 | {} |
| Input Variety | {}/25 | {} |"#,
            result.breakdown.assertion_quality,
            Self::score_status(result.breakdown.assertion_quality),
            result.breakdown.error_coverage,
            Self::score_status(result.breakdown.error_coverage),
            result.breakdown.boundary_conditions,
            Self::score_status(result.breakdown.boundary_conditions),
            result.breakdown.test_isolation,
            Self::score_status(result.breakdown.test_isolation),
            result.breakdown.input_variety,
            Self::score_status(result.breakdown.input_variety),
        )
    }

    fn score_status(score: u8) -> &'static str {
        match score {
            23..=25 => "✅ Excellent",
            18..=22 => "👍 Good",
            12..=17 => "⚠️ Needs Work",
            _ => "❌ Poor",
        }
    }

    fn format_critical_fixes(&self, issues: &[Issue]) -> String {
        let critical: Vec<_> = issues
            .iter()
            .filter(|i| {
                i.severity == Severity::Error
                    || (i.severity == Severity::Warning
                        && matches!(i.rule, Rule::NoAssertions | Rule::WeakAssertion))
            })
            .collect();

        if critical.is_empty() {
            return "No critical issues to fix.".to_string();
        }

        let mut output = String::new();
        for issue in critical.iter().take(5) {
            output.push_str(&format!(
                "- Line {}: {} → {}\n",
                issue.location.line,
                issue.rule,
                issue.suggestion.as_deref().unwrap_or("Fix required")
            ));
        }
        output
    }

    fn format_missing_tests(&self, issues: &[Issue]) -> String {
        let missing: Vec<_> = issues
            .iter()
            .filter(|i| {
                matches!(
                    i.rule,
                    Rule::MissingErrorTest | Rule::MissingBoundaryTest | Rule::LimitedInputVariety
                )
            })
            .collect();

        if missing.is_empty() {
            return "Coverage looks good. Consider adding edge cases.".to_string();
        }

        let mut output = String::new();
        for issue in missing.iter().take(5) {
            output.push_str(&format!("- {}\n", issue.message));
        }
        output
    }

    fn extract_context(&self, content: &str, line: usize, context_lines: usize) -> String {
        let lines: Vec<&str> = content.lines().collect();
        let start = line.saturating_sub(context_lines + 1);
        let end = (line + context_lines).min(lines.len());

        lines[start..end]
            .iter()
            .enumerate()
            .map(|(i, l)| format!("{:4} | {}", start + i + 1, l))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl Default for AiSuggestionGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Location, Rule, Score, ScoreBreakdown, TestFramework, TestStats, TestType};
    use std::path::PathBuf;

    #[test]
    fn test_generate_prompt() {
        let result = AnalysisResult {
            file_path: PathBuf::from("nonexistent.test.ts"),
            score: Score::new(60),
            breakdown: ScoreBreakdown {
                assertion_quality: 15,
                error_coverage: 10,
                boundary_conditions: 12,
                test_isolation: 13,
                input_variety: 10,
            },
            issues: vec![Issue {
                rule: Rule::WeakAssertion,
                severity: Severity::Warning,
                message: "Weak assertion found".to_string(),
                location: Location::new(5, 1),
                suggestion: Some("Use toBe() instead".to_string()),
            }],
            stats: TestStats::default(),
            framework: TestFramework::Jest,
            test_type: TestType::Unit,
            source_file: None,
        };

        let generator = AiSuggestionGenerator::new();
        let prompt = generator.generate_prompt(&result);

        assert!(prompt.contains("**File:**"));
        assert!(prompt.contains("60/100"));
        assert!(prompt.contains("Weak assertion") || prompt.contains("weak-assertion"));
    }
}
