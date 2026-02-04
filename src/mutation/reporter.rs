//! Report mutation testing results.

use super::relevance::relevance_summary;
use super::MutationResult;
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
    println!("   Mutants: {} total, {} killed, {} survived", total, killed, survived);
    println!("   Score: {}%", pct);

    if !result.details.is_empty() {
        println!();
        println!("   {}", "Details:".bold());
        for run in &result.details {
            let status = if run.killed { "KILLED".green() } else { "SURVIVED".red() };
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
