//! Run tests against each mutant.

use super::operators::{apply_mutation, Mutation};
use std::fs;
use std::path::Path;
use std::process::Command;

use super::MutationRun;

/// Run tests for each mutation: overwrite source, run cmd, restore.
pub fn run_mutations(
    source_path: &Path,
    original_content: &str,
    mutations: &[Mutation],
    test_command: &str,
) -> std::io::Result<Vec<MutationRun>> {
    let mut results = Vec::with_capacity(mutations.len());

    for mutation in mutations {
        let mutated_content = apply_mutation(original_content, mutation);
        if mutated_content == original_content {
            continue;
        }

        if let Err(e) = fs::write(source_path, &mutated_content) {
            results.push(MutationRun {
                mutation: mutation.clone(),
                killed: false,
                stdout: String::new(),
                stderr: format!("Failed to write: {}", e),
            });
            continue;
        }

        let (killed, stdout, stderr) =
            run_test_command(test_command, source_path.parent().unwrap_or(Path::new(".")));

        if fs::write(source_path, original_content).is_err() {
            // Best-effort restore; continue
        }

        results.push(MutationRun {
            mutation: mutation.clone(),
            killed,
            stdout,
            stderr,
        });
    }

    Ok(results)
}

fn run_test_command(cmd: &str, cwd: &Path) -> (bool, String, String) {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    let (binary, args) = if parts.is_empty() {
        ("npm", vec!["test"])
    } else {
        (parts[0], parts[1..].to_vec())
    };

    let output = Command::new(binary).args(args).current_dir(cwd).output();

    match output {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout).to_string();
            let stderr = String::from_utf8_lossy(&o.stderr).to_string();
            let killed = !o.status.success();
            (killed, stdout, stderr)
        }
        Err(e) => (false, String::new(), format!("{}", e)),
    }
}
