//! Fast mutation mode - run a small set of strategic mutants and report kill rate.

mod operators;
mod reporter;
mod runner;
mod sampler;

pub use operators::{apply_mutation, Mutation, MutationOperator};
pub use reporter::report as report_mutation_result;

use std::path::Path;

/// Result of running mutation testing on a source file
#[derive(Debug)]
pub struct MutationResult {
    /// Path to the source file that was mutated
    pub source_path: std::path::PathBuf,
    /// Total mutants generated
    pub total: usize,
    /// Number of mutants killed (tests failed)
    pub killed: usize,
    /// Number of mutants that survived (tests passed)
    pub survived: usize,
    /// Details per mutation
    pub details: Vec<MutationRun>,
}

/// Outcome of running tests against one mutant
#[derive(Debug, Clone)]
pub struct MutationRun {
    pub mutation: Mutation,
    pub killed: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Run mutation testing: generate mutants, run tests, report.
pub fn run_mutation_test(
    source_path: &Path,
    source_content: &str,
    test_command: &str,
    count: usize,
) -> std::io::Result<MutationResult> {
    let mutations = operators::generate_mutations(source_content);
    let selected = sampler::select_mutations(&mutations, count);
    let runs = runner::run_mutations(source_path, source_content, &selected, test_command)?;
    let killed = runs.iter().filter(|r| r.killed).count();
    let survived = runs.len() - killed;

    Ok(MutationResult {
        source_path: source_path.to_path_buf(),
        total: selected.len(),
        killed,
        survived,
        details: runs,
    })
}
