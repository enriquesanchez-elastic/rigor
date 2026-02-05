//! Report mutation testing results.

use super::relevance::relevance_summary;
use super::{BatchMutationResult, MutationResult};
use colored::Colorize;

/// Print mutation result to stdout, including a relevance section when mutants survived.
pub fn report(result: &MutationResult) {
    let total = result.total;
    let killed = result.killed;
    let survived = result.survived;
    let pct = if total > 0 {
        (killed as f32 / total as f32 * 100.0) as u32
    } else {
        0
    };

    println!();
    println!("{}", "Mutation testing".bold());
    println!("   Source: {}", result.source_path.display());
    println!(
        "   Mutants: {} total, {} killed, {} survived",
        total, killed, survived
    );
    println!("   Score: {}%", pct);

    if !result.details.is_empty() {
        println!();
        println!("   {}", "Details:".bold());
        for run in &result.details {
            let status = if run.killed {
                "KILLED".green()
            } else {
                "SURVIVED".red()
            };
            println!(
                "   {} L{}:{} {} → {}",
                status,
                run.mutation.line,
                run.mutation.column,
                run.mutation.original.dimmed(),
                run.mutation.replacement.dimmed()
            );
        }
    }

    // Relevance: interpret survived mutants as "tests not relevant to these behaviors"
    if survived > 0 {
        let rel = relevance_summary(result);
        println!();
        println!("   {}", "Test relevance".bold());
        println!(
            "   {} source line(s) had at least one survived mutant → tests did not catch these changes.",
            rel.lines_with_survived
        );
        let mut lines: Vec<_> = rel.survived_by_line.keys().copied().collect();
        lines.sort_unstable();
        for line in lines {
            let at = rel.survived_by_line.get(&line).unwrap();
            let descs: Vec<_> = at.iter().map(|s| s.description.as_str()).collect();
            println!("   L{}: {} ({} survived)", line, descs.join("; "), at.len());
        }
        if !rel.suggestions.is_empty() {
            println!();
            println!("   {}", "Suggestions:".bold());
            for s in &rel.suggestions {
                println!("   • {}", s);
            }
        }
    }

    println!();
}

/// Print batch mutation results to stdout
pub fn report_batch(result: &BatchMutationResult) {
    println!();
    println!("{}", "Batch Mutation Testing Results".bold());
    println!("   Files analyzed: {}", result.source_results.len());
    println!("   Total mutants: {}", result.total_mutants);
    println!(
        "   Killed: {} ({}%), Survived: {}",
        result.total_killed, result.overall_score as u32, result.total_survived
    );
    println!(
        "   {}: {:.1}%",
        "Overall Score".bold(),
        result.overall_score
    );

    // Per-file breakdown
    if !result.source_results.is_empty() {
        println!();
        println!("   {}", "Per-file breakdown:".bold());
        for file_result in &result.source_results {
            let pct = file_result.score() as u32;
            let status = if pct >= 80 {
                format!("{}%", pct).green()
            } else if pct >= 60 {
                format!("{}%", pct).yellow()
            } else {
                format!("{}%", pct).red()
            };
            println!(
                "   {} {} ({}/{} killed)",
                status,
                file_result.source_path.display(),
                file_result.killed,
                file_result.total
            );
        }
    }

    // Find worst performing files
    let mut files_with_survivors: Vec<_> = result
        .source_results
        .iter()
        .filter(|r| r.survived > 0)
        .collect();
    files_with_survivors.sort_by(|a, b| b.survived.cmp(&a.survived));

    if !files_with_survivors.is_empty() {
        println!();
        println!("   {}", "Files needing attention:".bold());
        for file_result in files_with_survivors.iter().take(5) {
            println!(
                "   • {} - {} survivors",
                file_result
                    .source_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "?".to_string()),
                file_result.survived
            );
        }
    }

    println!();
}
