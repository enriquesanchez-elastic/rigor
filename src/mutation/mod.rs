//! Fast mutation mode - run a small set of strategic mutants and report kill rate.

mod operators;
mod relevance;
mod reporter;
mod runner;
mod sampler;

pub use operators::{apply_mutation, Mutation, MutationOperator};
pub use relevance::{relevance_summary, RelevanceSummary, SurvivedAtLine};
pub use reporter::report as report_mutation_result;
pub use reporter::report_batch as report_batch_mutation_result;

use std::path::{Path, PathBuf};

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

impl MutationResult {
    /// Calculate mutation score as percentage
    pub fn score(&self) -> f32 {
        if self.total == 0 {
            100.0
        } else {
            (self.killed as f32 / self.total as f32) * 100.0
        }
    }
}

/// Result of running mutation testing on multiple source files
#[derive(Debug)]
pub struct BatchMutationResult {
    /// Results for each source file
    pub source_results: Vec<MutationResult>,
    /// Overall mutation score (0-100)
    pub overall_score: f32,
    /// Total mutants across all files
    pub total_mutants: usize,
    /// Total killed mutants across all files
    pub total_killed: usize,
    /// Total survived mutants across all files
    pub total_survived: usize,
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

/// Run mutation testing on multiple source files
pub fn run_batch_mutation_test(
    source_paths: &[PathBuf],
    test_command: &str,
    count_per_file: usize,
    parallel: bool,
) -> std::io::Result<BatchMutationResult> {
    let results: Vec<MutationResult> = if parallel {
        use rayon::prelude::*;
        source_paths
            .par_iter()
            .filter_map(|path| {
                let content = std::fs::read_to_string(path).ok()?;
                run_mutation_test(path, &content, test_command, count_per_file).ok()
            })
            .collect()
    } else {
        source_paths
            .iter()
            .filter_map(|path| {
                let content = std::fs::read_to_string(path).ok()?;
                run_mutation_test(path, &content, test_command, count_per_file).ok()
            })
            .collect()
    };

    let total_mutants: usize = results.iter().map(|r| r.total).sum();
    let total_killed: usize = results.iter().map(|r| r.killed).sum();
    let total_survived = total_mutants - total_killed;

    let overall_score = if total_mutants == 0 {
        100.0
    } else {
        (total_killed as f32 / total_mutants as f32) * 100.0
    };

    Ok(BatchMutationResult {
        source_results: results,
        overall_score,
        total_mutants,
        total_killed,
        total_survived,
    })
}
