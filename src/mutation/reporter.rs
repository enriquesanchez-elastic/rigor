//! Report mutation testing results.

use super::MutationResult;
use colored::Colorize;

/// Print mutation result to stdout.
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
    println!();
}
