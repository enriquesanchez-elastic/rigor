//! Auto-fix application for fixable rules (e.g. focused-test, debug-code).

use crate::{Fix, Issue, Rule};
use std::fs;
use std::path::Path;

/// Compute a fix for an issue if the rule is auto-fixable (e.g. focused-test, debug-code).
/// Returns None if the rule has no fix or the fix cannot be determined.
pub fn fix_for_issue(issue: &Issue, line_content: &str, line_no: usize) -> Option<Fix> {
    match issue.rule {
        Rule::FocusedTest => fix_focused_test(line_content, line_no),
        Rule::DebugCode => fix_debug_code(line_content, line_no, &issue.message),
        _ => None,
    }
}

fn fix_focused_test(line: &str, line_no: usize) -> Option<Fix> {
    let trimmed = line.trim_start();
    let start_col = line.len() - trimmed.len() + 1;

    let (from, to) = if trimmed.contains("it.only(") {
        ("it.only(", "it(")
    } else if trimmed.contains("test.only(") {
        ("test.only(", "test(")
    } else if trimmed.contains("fit(") {
        ("fit(", "it(")
    } else if trimmed.contains("ftest(") {
        ("ftest(", "test(")
    } else if trimmed.contains("fdescribe(") {
        ("fdescribe(", "describe(")
    } else if trimmed.contains("describe.only(") {
        ("describe.only(", "describe(")
    } else {
        return None;
    };

    let col = trimmed.find(from)? + start_col;
    let end_col = col + from.len();
    Some(Fix {
        start_line: line_no,
        start_column: col,
        end_line: line_no,
        end_column: end_col,
        replacement: to.to_string(),
    })
}

fn fix_debug_code(line: &str, line_no: usize, message: &str) -> Option<Fix> {
    let trimmed = line.trim_start();
    let start_col = line.len() - trimmed.len() + 1;

    if message.contains("debugger") {
        let col = trimmed.find("debugger")? + start_col;
        let end_col = col + "debugger".len();
        return Some(Fix {
            start_line: line_no,
            start_column: col,
            end_line: line_no,
            end_column: end_col,
            replacement: String::new(),
        });
    }
    if message.contains("console.log") && trimmed.contains("console.log(") {
        // Remove whole line (simplest): replace line with empty or whitespace
        let col = trimmed.find("console.log")? + start_col;
        let rest = &line[col - 1..];
        let end = rest
            .find(';')
            .map(|i| col + i)
            .unwrap_or_else(|| line.len() + 1);
        return Some(Fix {
            start_line: line_no,
            start_column: col,
            end_line: line_no,
            end_column: end,
            replacement: String::new(),
        });
    }
    None
}

/// Apply a list of fixes to a file. Fixes are applied from bottom to top so offsets remain valid.
pub fn apply_fixes(path: &Path, fixes: &[Fix]) -> std::io::Result<()> {
    if fixes.is_empty() {
        return Ok(());
    }
    let mut sorted: Vec<&Fix> = fixes.iter().collect();
    sorted.sort_by(|a, b| (b.start_line, b.start_column).cmp(&(a.start_line, a.start_column)));
    let content = fs::read_to_string(path)?;
    let mut new_content = content.clone();
    let lines: Vec<&str> = content.lines().collect();
    for fix in sorted {
        let start_byte = line_col_to_byte(&lines, fix.start_line, fix.start_column);
        let end_byte = line_col_to_byte(&lines, fix.end_line, fix.end_column);
        if start_byte <= end_byte && end_byte <= new_content.len() {
            new_content.replace_range(start_byte..end_byte, &fix.replacement);
        }
    }
    fs::write(path, new_content)
}

fn line_col_to_byte(lines: &[&str], line: usize, col: usize) -> usize {
    let mut byte = 0;
    for (i, l) in lines.iter().enumerate() {
        let line_no = i + 1;
        if line_no < line {
            byte += l.len() + 1;
        } else if line_no == line {
            byte += (col - 1).min(l.len());
            break;
        }
    }
    byte
}

/// Collect all fixes from analysis results (issues with fix populated)
pub fn collect_fixes_from_results(
    results: &[crate::AnalysisResult],
) -> Vec<(std::path::PathBuf, Fix)> {
    let mut out = Vec::new();
    for r in results {
        for issue in &r.issues {
            if let Some(ref fix) = issue.fix {
                out.push((r.file_path.clone(), fix.clone()));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fix_focused_it_only() {
        let line = "    it.only('foo', () => {});";
        let fix = fix_focused_test(line, 5).unwrap();
        assert_eq!(fix.replacement, "it(");
        assert_eq!(fix.start_line, 5);
        assert_eq!(fix.end_column, fix.start_column + "it.only(".len());
    }

    #[test]
    fn fix_focused_fit() {
        let line = "  fit('bar', () => {});";
        let fix = fix_focused_test(line, 1).unwrap();
        assert_eq!(fix.replacement, "it(");
    }
}
