//! Console reporter with colored output

use crate::analyzer::engine::AggregateStats;
use crate::analyzer::scoring::ScoreCalculator;
use crate::{rule_scoring_category, AnalysisResult, Grade, Issue, Severity, TestScore};
use colored::Colorize;

/// Reporter for terminal output
pub struct ConsoleReporter {
    /// Whether to use colors
    use_colors: bool,
    /// Whether to show verbose output
    verbose: bool,
}

impl ConsoleReporter {
    /// Create a new console reporter
    pub fn new() -> Self {
        Self {
            use_colors: true,
            verbose: false,
        }
    }

    /// Disable colors
    pub fn without_colors(mut self) -> Self {
        self.use_colors = false;
        self
    }

    /// Enable verbose output
    pub fn verbose(mut self) -> Self {
        self.verbose = true;
        self
    }

    /// Report a single analysis result
    pub fn report(&self, result: &AnalysisResult) {
        self.print_header(result);
        self.print_score(result);
        self.print_breakdown(result);

        if let Some(ref test_scores) = result.test_scores {
            self.print_test_scores_weakest_first(test_scores);
        }

        if !result.issues.is_empty() {
            self.print_issues(result);
        }

        self.print_recommendations(result);
        println!();
    }

    /// Report multiple results with summary
    pub fn report_many(&self, results: &[AnalysisResult], stats: &AggregateStats) {
        for result in results {
            self.report(result);
            println!("{}", "─".repeat(60));
        }

        self.print_summary(stats);
    }

    /// Report in quiet mode (just score)
    pub fn report_quiet(&self, result: &AnalysisResult) {
        let grade_colored = self.colorize_grade(&result.score.grade);
        println!(
            "{}: {} ({})",
            result.file_path.display(),
            result.score.value,
            grade_colored
        );
    }

    fn print_header(&self, result: &AnalysisResult) {
        println!();
        println!(
            "{}",
            format!("📊 Test Quality Analysis: {}", result.file_path.display()).bold()
        );
        println!(
            "   Framework: {} | Tests: {} | Assertions: {}",
            result.framework, result.stats.total_tests, result.stats.total_assertions
        );
        if let Some(ref source) = result.source_file {
            println!("   Source: {}", source.display());
        }
        println!();
    }

    fn print_score(&self, result: &AnalysisResult) {
        let grade_str = self.colorize_grade(&result.score.grade);
        let score_bar = self.create_score_bar(result.score.value);

        println!("   Score: {} {}", score_bar, grade_str.bold());
        println!(
            "   {}",
            ScoreCalculator::grade_description(result.score.grade).dimmed()
        );
        println!();
    }

    fn print_breakdown(&self, result: &AnalysisResult) {
        println!("   {}", "Score Breakdown:".bold());

        if let Some(ref tb) = result.transparent_breakdown {
            for cat in &tb.categories {
                let bar = self.create_mini_bar(cat.raw_score, cat.max_raw);
                let score_str = format!("{:>2}/{}", cat.raw_score, cat.max_raw);
                let colored_score = if cat.raw_score >= 20 {
                    score_str.green()
                } else if cat.raw_score >= 15 {
                    score_str.yellow()
                } else {
                    score_str.red()
                };
                println!(
                    "   {} {} {} (weight {}%, contributes {})",
                    bar,
                    colored_score,
                    cat.category_name,
                    cat.weight_pct,
                    cat.weighted_contribution
                );
            }
            for line in Self::format_breakdown_summary(tb) {
                println!("   {}", line);
            }
        } else {
            let categories = [
                ("Assertion Quality", result.breakdown.assertion_quality),
                ("Error Coverage", result.breakdown.error_coverage),
                ("Boundary Conditions", result.breakdown.boundary_conditions),
                ("Test Isolation", result.breakdown.test_isolation),
                ("Input Variety", result.breakdown.input_variety),
            ];
            for (name, score) in categories {
                let bar = self.create_mini_bar(score, 25);
                let score_str = format!("{:>2}/25", score);
                let colored_score = if score >= 20 {
                    score_str.green()
                } else if score >= 15 {
                    score_str.yellow()
                } else {
                    score_str.red()
                };
                println!("   {} {} {}", bar, colored_score, name);
            }
        }
        println!();
    }

    /// Format the breakdown summary lines (penalties + per-test aggregation).
    /// Returns plain-text lines without color codes for testability.
    ///
    /// Three display paths:
    /// 1. **Per-test pulled down**: per-test average < penalty-adjusted score → score reduced
    /// 2. **Per-test capped**: per-test average > penalty-adjusted score → capped by breakdown
    /// 3. **Simple penalty**: no per-test aggregation effect, just penalty math
    fn format_breakdown_summary(tb: &crate::TransparentBreakdown) -> Vec<String> {
        let after_penalty = (tb.total_before_penalties as i32 - tb.penalty_total).max(0) as u8;
        let mut lines = Vec::new();

        if let Some(aggregated) = tb.per_test_aggregated {
            lines.push(format!(
                "{} before penalties, −{} penalty → {}",
                tb.total_before_penalties, tb.penalty_total, after_penalty
            ));
            if aggregated < after_penalty {
                lines.push(format!(
                    "per-test average {} → final {}",
                    aggregated, tb.final_score
                ));
            } else {
                lines.push(format!(
                    "per-test average {} capped by breakdown → final {}",
                    aggregated, tb.final_score
                ));
            }
        } else {
            lines.push(format!(
                "{} before penalties, −{} penalty → {}",
                tb.total_before_penalties, tb.penalty_total, tb.final_score
            ));
        }
        lines
    }

    fn print_test_scores_weakest_first(&self, test_scores: &[TestScore]) {
        if test_scores.is_empty() {
            return;
        }
        println!("   {}", "Per-test scores (weakest first):".bold());
        let mut sorted: Vec<&TestScore> = test_scores.iter().collect();
        sorted.sort_by(|a, b| a.score.cmp(&b.score).then_with(|| a.line.cmp(&b.line)));
        for ts in sorted {
            let grade_str = self.colorize_grade(&ts.grade);
            let line_info = ts
                .end_line
                .map(|e| format!("L{}-{}", ts.line, e))
                .unwrap_or_else(|| format!("L{}", ts.line));
            println!(
                "   {} {} {} {}",
                line_info.dimmed(),
                grade_str,
                ts.score,
                ts.name
            );
        }
        println!();
    }

    fn print_issues(&self, result: &AnalysisResult) {
        println!("   {}", "Issues Found:".bold());

        // Group by severity
        let errors: Vec<_> = result
            .issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .collect();
        let warnings: Vec<_> = result
            .issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
            .collect();
        let infos: Vec<_> = result
            .issues
            .iter()
            .filter(|i| i.severity == Severity::Info)
            .collect();

        for issue in errors {
            self.print_issue(issue);
        }
        for issue in warnings {
            self.print_issue(issue);
        }

        // Only show info issues in verbose mode or if there are few total issues
        if self.verbose || result.issues.len() <= 5 {
            for issue in infos {
                self.print_issue(issue);
            }
        } else if !infos.is_empty() {
            println!(
                "   {} {} additional suggestions (use --verbose to show)",
                "ℹ".blue(),
                infos.len()
            );
        }

        println!();
    }

    fn print_issue(&self, issue: &Issue) {
        let icon = match issue.severity {
            Severity::Error => "✗".red(),
            Severity::Warning => "⚠".yellow(),
            Severity::Info => "ℹ".blue(),
        };

        let location = format!("L{}:{}", issue.location.line, issue.location.column);
        println!(
            "   {} {} [{}] {}",
            icon,
            location.dimmed(),
            issue.rule.to_string().dimmed(),
            issue.message
        );

        if self.verbose {
            let category_note = match rule_scoring_category(&issue.rule) {
                Some(cat) => format!("affects category: {}", cat),
                None => "affects penalty only".to_string(),
            };
            println!("       {} {}", "↳".dimmed(), category_note.dimmed());
        }

        if let Some(ref suggestion) = issue.suggestion {
            let arrow = "→".dimmed();
            if suggestion.contains(';') && !suggestion.contains('\n') {
                for line in suggestion.split("; ") {
                    println!("       {} {}", arrow, line.trim().italic());
                }
            } else {
                println!("       {} {}", arrow, suggestion.italic());
            }
        }
    }

    fn print_recommendations(&self, result: &AnalysisResult) {
        let recs =
            ScoreCalculator::recommendations(&result.breakdown, &result.issues, result.score.grade);

        if result.score.value < 90 {
            println!("   {}", "Recommendations:".bold());
            for rec in recs.iter().take(3) {
                println!("   {} {}", "→".cyan(), rec);
            }
        }
    }

    fn print_summary(&self, stats: &AggregateStats) {
        println!();
        println!("{}", "═".repeat(60));
        println!("{}", "Summary".bold());
        println!("{}", "═".repeat(60));
        println!(
            "   Files analyzed: {}",
            stats.files_analyzed.to_string().bold()
        );
        println!(
            "   Average score:  {} ({})",
            stats.average_score.value.to_string().bold(),
            self.colorize_grade(&stats.average_score.grade)
        );
        println!("   Total tests:    {}", stats.total_tests);
        println!("   Total issues:   {}", stats.total_issues);
        println!();
    }

    fn colorize_grade(&self, grade: &Grade) -> colored::ColoredString {
        let s = grade.to_string();
        match grade {
            Grade::A => s.green().bold(),
            Grade::B => s.green(),
            Grade::C => s.yellow(),
            Grade::D => s.red(),
            Grade::F => s.red().bold(),
        }
    }

    fn create_score_bar(&self, score: u8) -> String {
        let filled = (score as usize * 20) / 100;
        let empty = 20 - filled;

        let bar = format!(
            "[{}{}] {:>3}%",
            "█".repeat(filled),
            "░".repeat(empty),
            score
        );

        if self.use_colors {
            if score >= 80 {
                bar.green().to_string()
            } else if score >= 60 {
                bar.yellow().to_string()
            } else {
                bar.red().to_string()
            }
        } else {
            bar
        }
    }

    fn create_mini_bar(&self, score: u8, max: u8) -> String {
        let filled = (score as usize * 10) / max as usize;
        let empty = 10 - filled;
        format!("[{}{}]", "▓".repeat(filled), "░".repeat(empty))
    }
}

impl Default for ConsoleReporter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TransparentBreakdown;

    fn make_tb(
        total_before: u8,
        penalty: i32,
        final_score: u8,
        per_test_agg: Option<u8>,
    ) -> TransparentBreakdown {
        TransparentBreakdown {
            categories: vec![],
            total_before_penalties: total_before,
            penalty_total: penalty,
            penalty_from_errors: 0,
            penalty_from_warnings: 0,
            penalty_from_info: 0,
            final_score,
            per_test_aggregated: per_test_agg,
        }
    }

    /// Path 1: Per-test average pulled score DOWN (e.g. no-assertion tests floor at 30).
    /// Shows: "90 before penalties, −0 penalty → 90" then "per-test average 50 → final 50"
    #[test]
    fn breakdown_summary_per_test_pulled_down() {
        // total_before=90, penalty=0 → after_penalty=90, aggregated=50 < 90
        let tb = make_tb(90, 0, 50, Some(50));
        let lines = ConsoleReporter::format_breakdown_summary(&tb);

        assert_eq!(lines.len(), 2, "pulled-down path should have 2 lines");
        assert!(
            lines[0].contains("90 before penalties"),
            "first line: {}",
            lines[0]
        );
        assert!(
            lines[1].contains("per-test average 50") && lines[1].contains("→ final 50"),
            "should show per-test average pulling score down: {}",
            lines[1]
        );
        // Must NOT contain "capped"
        assert!(
            !lines[1].contains("capped"),
            "pulled-down path should not say 'capped': {}",
            lines[1]
        );
    }

    /// Path 2: Per-test average higher than breakdown, capped by breakdown.
    /// Shows: "80 before penalties, −5 penalty → 75" then "per-test average 85 capped by breakdown → final 75"
    #[test]
    fn breakdown_summary_per_test_capped() {
        // total_before=80, penalty=5 → after_penalty=75, aggregated=85 > 75
        let tb = make_tb(80, 5, 75, Some(85));
        let lines = ConsoleReporter::format_breakdown_summary(&tb);

        assert_eq!(lines.len(), 2, "capped path should have 2 lines");
        assert!(
            lines[0].contains("80 before penalties") && lines[0].contains("−5 penalty"),
            "first line: {}",
            lines[0]
        );
        assert!(
            lines[1].contains("per-test average 85")
                && lines[1].contains("capped by breakdown")
                && lines[1].contains("→ final 75"),
            "should show per-test average capped: {}",
            lines[1]
        );
    }

    /// Path 3: Simple penalty — no per-test aggregation effect.
    /// Shows: "85 before penalties, −10 penalty → 75"
    #[test]
    fn breakdown_summary_simple_penalty() {
        let tb = make_tb(85, 10, 75, None);
        let lines = ConsoleReporter::format_breakdown_summary(&tb);

        assert_eq!(lines.len(), 1, "simple penalty path should have 1 line");
        assert!(
            lines[0].contains("85 before penalties")
                && lines[0].contains("−10 penalty")
                && lines[0].contains("→ 75"),
            "should show simple penalty math: {}",
            lines[0]
        );
    }

    /// Edge case: zero penalties and no per-test aggregation.
    #[test]
    fn breakdown_summary_no_penalties_no_aggregation() {
        let tb = make_tb(92, 0, 92, None);
        let lines = ConsoleReporter::format_breakdown_summary(&tb);

        assert_eq!(lines.len(), 1);
        assert!(
            lines[0].contains("92 before penalties")
                && lines[0].contains("−0 penalty")
                && lines[0].contains("→ 92"),
            "should show clean pass-through: {}",
            lines[0]
        );
    }
}
