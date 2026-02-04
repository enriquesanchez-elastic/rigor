//! Rigor: Test Quality Analyzer CLI

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use rigor::cache::AnalysisCache;
use rigor::config::{build_ignore_set, is_ignored, load_config, CONFIG_FILENAME};
use rigor::analyzer::AnalysisEngine;
use rigor::mutation::{self, report_mutation_result};
use rigor::reporter::{ConsoleReporter, JsonReporter, SarifReporter};
use rigor::suggestions::{extract_code_block, offer_apply, AiSuggestionGenerator};
use rigor::history::{append_run, find_project_root, format_delta, load_history, previous_score, save_history};
use rigor::watcher::TestWatcher;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use walkdir::WalkDir;

/// Rigor: Test Quality Analyzer for TypeScript
#[derive(Parser, Debug)]
#[command(name = "rigor")]
#[command(author, version, about, long_about = None)]
#[command(args_conflicts_with_subcommands = true)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Test file or directory to analyze (omit when using a subcommand)
    #[arg(required_unless_present = "command")]
    path: Option<PathBuf>,

    /// Output format as JSON
    #[arg(long, short)]
    json: bool,

    /// Minimum score threshold (exit 1 if below)
    #[arg(long, short)]
    threshold: Option<u8>,

    /// Quiet mode (minimal output)
    #[arg(long, short)]
    quiet: bool,

    /// Verbose output
    #[arg(long, short)]
    verbose: bool,

    /// Generate AI improvement prompt
    #[arg(long)]
    fix: bool,

    /// With --fix: run RIGOR_APPLY_CMD with prompt on stdin and apply suggested code (prompts for confirmation)
    #[arg(long)]
    apply: bool,

    /// Output file for AI prompt (default: stdout)
    #[arg(long)]
    fix_output: Option<PathBuf>,

    /// Disable source file analysis
    #[arg(long)]
    no_source: bool,

    /// Path to config file (default: search .rigorrc.json in current dir and parents)
    #[arg(long)]
    config: Option<PathBuf>,

    /// Watch for file changes and re-analyze
    #[arg(long)]
    watch: bool,

    /// Output in SARIF format (for GitHub Code Scanning)
    #[arg(long)]
    sarif: bool,

    /// Only analyze staged (git) test files (for pre-commit hooks)
    #[arg(long)]
    staged: bool,

    /// Run fast mutation testing (quick=10, medium=30, full=all). Requires source file.
    #[arg(long, value_name = "MODE", num_args = 0..=1, default_missing_value = "quick")]
    mutate: Option<Option<String>>,

    /// Only analyze files changed since last commit (git diff HEAD)
    #[arg(long)]
    changed: bool,

    /// Disable caching (re-analyze all files even if unchanged)
    #[arg(long)]
    no_cache: bool,

    /// Clear the analysis cache before running
    #[arg(long)]
    clear_cache: bool,

    /// Run analysis in parallel (default for directories with many files)
    #[arg(long)]
    parallel: bool,

    /// Number of parallel threads (default: number of CPU cores)
    #[arg(long, value_name = "N")]
    jobs: Option<usize>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run MCP server for Claude/Cursor (stdio JSON-RPC)
    Mcp,

    /// Create .rigorrc.json with sensible defaults
    Init {
        /// Minimum score threshold (e.g. 70)
        #[arg(long)]
        threshold: Option<u8>,

        /// Force framework: jest, vitest, playwright, cypress, mocha
        #[arg(long)]
        framework: Option<String>,

        /// Directory in which to create config (default: current)
        #[arg(long)]
        dir: Option<PathBuf>,
    },
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("{}: {}", "Error".red().bold(), e);
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<ExitCode> {
    let args = Args::parse();

    if let Some(cmd) = args.command {
        match cmd {
            Commands::Mcp => {
                return rigor::mcp::run_mcp_server()
                    .map_err(|e| anyhow::anyhow!("{}", e))
                    .and(Ok(ExitCode::SUCCESS));
            }
            Commands::Init { threshold, framework, dir } => {
                return run_init(threshold, framework, dir.as_deref());
            }
        }
    }

    let path = args.path.clone().expect("path required when not using subcommand");

    if args.watch {
        return run_watch(&args, &path);
    }

    // Resolve work directory for config search
    let work_dir = if path.is_file() {
        path.parent().unwrap_or(std::path::Path::new("."))
    } else {
        path.as_path()
    };

    // Load config (CLI flags override config file)
    let config = load_config(work_dir, args.config.as_deref())?
        .merge_with_cli(args.threshold, args.config.as_deref());

    // Build ignore set from config
    let ignore_set = if config.ignore.is_empty() {
        None
    } else {
        Some(build_ignore_set(&config.ignore)?)
    };

    // Collect test files: either staged, changed, or from path
    // Use test_root from config if set, otherwise use the CLI path
    let search_path = if let Some(ref test_root) = config.test_root {
        let root = if path.is_file() {
            path.parent().unwrap_or(Path::new("."))
        } else {
            path.as_path()
        };
        root.join(test_root)
    } else {
        path.clone()
    };

    let test_patterns = config.get_test_patterns();
    let test_files = if args.staged {
        let git_root = find_project_root(work_dir).unwrap_or_else(|| work_dir.to_path_buf());
        collect_staged_test_files(&git_root, ignore_set.as_ref(), &test_patterns)?
    } else if args.changed {
        let git_root = find_project_root(work_dir).unwrap_or_else(|| work_dir.to_path_buf());
        collect_changed_test_files(&git_root, ignore_set.as_ref(), &test_patterns)?
    } else {
        collect_test_files(&search_path, ignore_set.as_ref(), &test_patterns)?
    };

    if test_files.is_empty() {
        if args.staged || args.changed {
            if !args.quiet {
                eprintln!("{}: No changed test files to analyze", "Info".blue());
            }
            return Ok(ExitCode::SUCCESS);
        }
        eprintln!("{}: No test files found", "Warning".yellow());
        return Ok(ExitCode::from(2));
    }

    // Set up cache
    let project_root = find_project_root(work_dir).unwrap_or_else(|| work_dir.to_path_buf());
    let mut cache = if args.no_cache {
        AnalysisCache::disabled()
    } else {
        AnalysisCache::new(&project_root)
    };

    // Clear cache if requested
    if args.clear_cache {
        cache.clear();
        if !args.quiet {
            eprintln!("{}: Cache cleared", "Info".blue());
        }
    }

    // Set up parallel processing
    if let Some(jobs) = args.jobs {
        rayon::ThreadPoolBuilder::new()
            .num_threads(jobs)
            .build_global()
            .ok();
    }

    // Create analysis engine
    let engine = if args.no_source {
        AnalysisEngine::new().without_source_analysis()
    } else {
        AnalysisEngine::new()
    };

    // Determine if we should use parallel analysis
    let use_parallel = args.parallel || test_files.len() > 10;

    // Analyze files
    let (results, had_errors) = if use_parallel && !args.no_cache {
        analyze_files_parallel_cached(&engine, &test_files, &config, &cache, args.quiet)
    } else if use_parallel {
        analyze_files_parallel(&engine, &test_files, &config, args.quiet)
    } else {
        analyze_files_sequential_cached(&engine, &test_files, &config, &mut cache, args.quiet)
    };

    // Save cache
    if let Err(e) = cache.save() {
        if !args.quiet {
            eprintln!("{}: Failed to save cache: {}", "Warning".yellow(), e);
        }
    }

    if results.is_empty() {
        eprintln!("{}: All files failed to analyze", "Error".red());
        return Ok(ExitCode::from(2));
    }

    // Calculate aggregate stats
    let stats = AnalysisEngine::aggregate_stats(&results);

    // Output results
    if args.sarif {
        let reporter = SarifReporter::new();
        println!("{}", reporter.report(&results, Some(&stats)));
    } else if args.json {
        let reporter = JsonReporter::new().pretty();
        if results.len() == 1 {
            println!("{}", reporter.report(&results[0]));
        } else {
            println!("{}", reporter.report_with_summary(&results, &stats));
        }
    } else if args.quiet {
        let reporter = ConsoleReporter::new();
        let project_root = find_project_root(work_dir);
        let history = project_root.as_ref().map(|p| load_history(p.as_path()));
        for result in &results {
            if let Some(ref h) = history {
                let prev = previous_score(h, &result.file_path);
                let delta = format_delta(prev, result.score.value);
                println!(
                    "{}: {} ({}){}",
                    result.file_path.display(),
                    result.score.value,
                    result.score.grade,
                    delta
                );
            } else {
                reporter.report_quiet(result);
            }
        }
        if let Some(ref root) = project_root {
            let mut h = load_history(root.as_path());
            append_run(&mut h, &results, None);
            let _ = save_history(root, &h);
        }
    } else {
        let mut reporter = ConsoleReporter::new();
        if args.verbose {
            reporter = reporter.verbose();
        }

        if results.len() == 1 {
            reporter.report(&results[0]);
        } else {
            reporter.report_many(&results, &stats);
        }

        // Persist trend history
        if let Some(ref root) = find_project_root(work_dir) {
            let mut h = load_history(root.as_path());
            append_run(&mut h, &results, None);
            let _ = save_history(root, &h);
        }
    }

    // Run mutation testing if requested (single file with source only)
    if let Some(mutate_arg) = args.mutate {
        if results.len() == 1 {
            let result = &results[0];
            if let Some(ref source_path) = result.source_file {
                if source_path.exists() {
                    let count = match mutate_arg.as_deref().unwrap_or("quick") {
                        "full" => usize::MAX,
                        "medium" => 30,
                        _ => 10,
                    };
                    let test_cmd = std::env::var("RIGOR_TEST_CMD").unwrap_or_else(|_| "npm test".to_string());
                    if let Ok(content) = std::fs::read_to_string(source_path) {
                        if let Ok(mutation_result) =
                            mutation::run_mutation_test(source_path, &content, &test_cmd, count)
                        {
                            report_mutation_result(&mutation_result);
                        }
                    }
                }
            } else if !args.quiet {
                eprintln!("{}: --mutate requires a source file (analyzed file must map to a .ts source)", "Warning".yellow());
            }
        } else if !args.quiet {
            eprintln!("{}: --mutate only works with a single test file", "Warning".yellow());
        }
    }

    // Generate AI fix prompt and optionally apply
    if args.fix {
        if results.len() > 1 {
            eprintln!(
                "{}: --fix only works with a single file",
                "Warning".yellow()
            );
        } else {
            let generator = AiSuggestionGenerator::new();
            let prompt = generator.generate_prompt(&results[0]);
            let result = &results[0];

            if args.apply {
                // Try RIGOR_APPLY_CMD first
                let suggested = std::env::var("RIGOR_APPLY_CMD").ok().and_then(|cmd| {
                    let mut parts = cmd.split_whitespace();
                    let binary = parts.next()?;
                    let rest: Vec<&str> = parts.collect();
                    let mut child = std::process::Command::new(binary)
                        .args(rest)
                        .stdin(std::process::Stdio::piped())
                        .stdout(std::process::Stdio::piped())
                        .spawn()
                        .ok()?;
                    {
                        let stdin = child.stdin.as_mut()?;
                        std::io::Write::write_all(stdin, prompt.as_bytes()).ok()?;
                    }
                    let out = child.wait_with_output().ok()?;
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    extract_code_block(&stdout)
                });

                if let Some(suggested) = suggested {
                    let current = std::fs::read_to_string(&result.file_path).unwrap_or_default();
                    let _ = offer_apply(&result.file_path, &current, &suggested);
                } else {
                    // Try Claude API if available
                    let claude_result = try_claude_api(result, &prompt, args.quiet);
                    
                    if let Some(suggested) = claude_result {
                        let current = std::fs::read_to_string(&result.file_path).unwrap_or_default();
                        let _ = offer_apply(&result.file_path, &current, &suggested);
                    } else if !args.quiet {
                        eprintln!(
                            "\n{}: To auto-apply fixes, either:",
                            "Info".blue()
                        );
                        eprintln!("  1. Set ANTHROPIC_API_KEY for built-in Claude integration");
                        eprintln!("  2. Set RIGOR_APPLY_CMD to a custom command");
                        eprintln!("\nShowing prompt instead:\n");
                        println!("{}", "═".repeat(60));
                        println!("{}", "AI Improvement Prompt:".bold());
                        println!("{}", "═".repeat(60));
                        println!("{}", prompt);
                    }
                }
            } else if let Some(output_path) = args.fix_output {
                std::fs::write(&output_path, &prompt)
                    .with_context(|| format!("Failed to write prompt to {}", output_path.display()))?;
                if !args.quiet {
                    eprintln!(
                        "{}: AI prompt written to {}",
                        "Info".blue(),
                        output_path.display()
                    );
                }
            } else {
                println!("\n{}", "═".repeat(60));
                println!("{}", "AI Improvement Prompt:".bold());
                println!("{}", "═".repeat(60));
                println!("{}", prompt);
            }
        }
    }

    // Check threshold (config or CLI)
    let threshold = args.threshold.or(config.threshold);
    if let Some(threshold) = threshold {
        let score = if results.len() == 1 {
            results[0].score.value
        } else {
            stats.average_score.value
        };

        if score < threshold {
            if !args.quiet && !args.json {
                eprintln!(
                    "\n{}: Score {} is below threshold {}",
                    "Failed".red().bold(),
                    score,
                    threshold
                );
            }
            return Ok(ExitCode::from(1));
        }
    }

    if had_errors {
        Ok(ExitCode::from(2))
    } else {
        Ok(ExitCode::SUCCESS)
    }
}

fn run_init(
    threshold: Option<u8>,
    framework: Option<String>,
    dir: Option<&Path>,
) -> Result<ExitCode> {
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    let dir = dir.unwrap_or(&cwd);
    let config_path = dir.join(CONFIG_FILENAME);

    if config_path.exists() {
        eprintln!(
            "{}: {} already exists; use --dir to write elsewhere or remove it first",
            "Warning".yellow(),
            config_path.display()
        );
        return Ok(ExitCode::SUCCESS);
    }

    let framework_value = match framework.as_deref().map(str::to_lowercase).as_deref() {
        Some("jest") => "jest",
        Some("vitest") => "vitest",
        Some("playwright") => "playwright",
        Some("cypress") => "cypress",
        Some("mocha") => "mocha",
        _ => "auto",
    };

    let threshold_value = threshold.unwrap_or(70);

    let json = format!(
        r#"{{
  "threshold": {},
  "framework": "{}",
  "rules": {{
    "weak-assertion": "warning",
    "missing-error-test": "warning",
    "flaky-pattern": "warning",
    "snapshot-overuse": "off"
  }},
  "ignore": [
    "**/node_modules/**",
    "**/dist/**",
    "**/*.e2e.test.ts",
    "**/legacy/**"
  ],
  "sourceMapping": {{
    "mode": "auto"
  }},
  "overrides": [
    {{
      "files": ["**/*.e2e.test.ts", "**/*.e2e.spec.ts"],
      "skipSourceAnalysis": true
    }}
  ]
}}
"#,
        threshold_value,
        framework_value
    );
    // Note: Users can also add these options to the config:
    // - "testRoot": "tests" - directory to search for tests recursively
    // - "testPatterns": [".test.ts", ".spec.ts"] - custom test file patterns

    std::fs::write(&config_path, json).with_context(|| {
        format!("Failed to write config to {}", config_path.display())
    })?;

    println!(
        "{}: Created {} with threshold={}, framework={}",
        "Done".green().bold(),
        config_path.display(),
        threshold_value,
        framework_value
    );
    Ok(ExitCode::SUCCESS)
}

fn run_watch(args: &Args, path: &PathBuf) -> Result<ExitCode> {
    let work_dir = if path.is_file() {
        path.parent().unwrap_or(Path::new("."))
    } else {
        path.as_path()
    };

    let config = load_config(work_dir, args.config.as_deref())?
        .merge_with_cli(args.threshold, args.config.as_deref());
    let ignore_set = if config.ignore.is_empty() {
        None
    } else {
        Some(build_ignore_set(&config.ignore)?)
    };

    let engine = if args.no_source {
        AnalysisEngine::new().without_source_analysis()
    } else {
        AnalysisEngine::new()
    };

    let watcher = TestWatcher::watch(path).context("Failed to create file watcher")?;
    eprintln!("{}: Watching for changes... (Ctrl+C to stop)", "Info".blue());

    loop {
        let paths = watcher.next_changes();
        if paths.is_empty() {
            continue;
        }
        let filtered: Vec<PathBuf> = paths
            .into_iter()
            .filter(|p| {
                ignore_set
                    .as_ref()
                    .map(|set| !is_ignored(p, set))
                    .unwrap_or(true)
            })
            .collect();
        for path in filtered {
            match engine.analyze(&path, Some(&config)) {
                Ok(result) => {
                    if args.quiet {
                        ConsoleReporter::new().report_quiet(&result);
                    } else {
                        ConsoleReporter::new().report(&result);
                    }
                }
                Err(e) => {
                    eprintln!("{}: {}: {}", "Error".red(), path.display(), e);
                }
            }
        }
    }
}

/// Collect test file paths from git staged files.
fn collect_staged_test_files(
    work_dir: &Path,
    ignore_set: Option<&globset::GlobSet>,
    test_patterns: &[&str],
) -> Result<Vec<PathBuf>> {
    let output = std::process::Command::new("git")
        .args(["diff", "--cached", "--name-only"])
        .current_dir(work_dir)
        .output()
        .context("Failed to run git diff (is this a git repo?)")?;
    if !output.status.success() {
        anyhow::bail!("git diff --cached failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    let names = String::from_utf8_lossy(&output.stdout);
    let mut files = Vec::new();
    for line in names.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let candidate = work_dir.join(line);
        if is_test_file(&candidate, test_patterns) {
            if let Some(set) = ignore_set {
                if is_ignored(&candidate, set) {
                    continue;
                }
            }
            if candidate.exists() {
                files.push(candidate);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn collect_test_files(
    path: &PathBuf,
    ignore_set: Option<&globset::GlobSet>,
    test_patterns: &[&str],
) -> Result<Vec<PathBuf>> {
    if path.is_file() {
        if let Some(set) = ignore_set {
            if is_ignored(path, set) {
                return Ok(vec![]);
            }
        }
        return Ok(vec![path.clone()]);
    }

    if !path.is_dir() {
        anyhow::bail!("Path does not exist: {}", path.display());
    }

    let mut files = Vec::new();

    for entry in WalkDir::new(path)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let file_path = entry.path();
        if is_test_file(file_path, test_patterns) {
            if let Some(set) = ignore_set {
                if is_ignored(file_path, set) {
                    continue;
                }
            }
            files.push(file_path.to_path_buf());
        }
    }

    // Sort for consistent output
    files.sort();

    Ok(files)
}

fn is_test_file(path: &std::path::Path, test_patterns: &[&str]) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };

    // Skip node_modules
    if path
        .components()
        .any(|c| c.as_os_str() == "node_modules")
    {
        return false;
    }

    // Check for test file patterns from config
    test_patterns.iter().any(|p| name.ends_with(p))
}

/// Try to use Claude API for generating improved tests
fn try_claude_api(result: &rigor::AnalysisResult, _prompt: &str, quiet: bool) -> Option<String> {
    use rigor::suggestions::{ClaudeClient, is_ai_available};

    if !is_ai_available() {
        if !quiet {
            eprintln!(
                "{}: AI feature not enabled. Rebuild with: cargo build --features ai",
                "Note".blue()
            );
        }
        return None;
    }

    // Check if API key is available
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        return None;
    }

    if !quiet {
        eprintln!("{}: Calling Claude API...", "AI".cyan().bold());
    }

    match ClaudeClient::from_env() {
        Ok(client) => {
            match client.improve_tests(result) {
                Ok(response) => {
                    if !quiet {
                        if let Some(tokens) = response.tokens_used {
                            eprintln!("{}: Generated {} tokens", "AI".cyan().bold(), tokens);
                        }
                    }
                    Some(response.improved_code)
                }
                Err(e) => {
                    if !quiet {
                        eprintln!("{}: {}", "AI Error".red(), e);
                    }
                    None
                }
            }
        }
        Err(e) => {
            if !quiet {
                eprintln!("{}: {}", "AI Error".red(), e);
            }
            None
        }
    }
}

/// Collect test files changed since last commit (git diff HEAD)
fn collect_changed_test_files(
    work_dir: &Path,
    ignore_set: Option<&globset::GlobSet>,
    test_patterns: &[&str],
) -> Result<Vec<PathBuf>> {
    let output = std::process::Command::new("git")
        .args(["diff", "HEAD", "--name-only"])
        .current_dir(work_dir)
        .output()
        .context("Failed to run git diff (is this a git repo?)")?;
    
    // Also get untracked files
    let untracked = std::process::Command::new("git")
        .args(["ls-files", "--others", "--exclude-standard"])
        .current_dir(work_dir)
        .output()
        .ok();

    let mut all_files = String::from_utf8_lossy(&output.stdout).to_string();
    if let Some(ut) = untracked {
        if ut.status.success() {
            all_files.push_str(&String::from_utf8_lossy(&ut.stdout));
        }
    }

    let mut files = Vec::new();
    for line in all_files.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let candidate = work_dir.join(line);
        if is_test_file(&candidate, test_patterns) {
            if let Some(set) = ignore_set {
                if is_ignored(&candidate, set) {
                    continue;
                }
            }
            if candidate.exists() {
                files.push(candidate);
            }
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
}

/// Analyze files sequentially with caching
fn analyze_files_sequential_cached(
    engine: &AnalysisEngine,
    files: &[PathBuf],
    config: &rigor::config::Config,
    cache: &mut AnalysisCache,
    quiet: bool,
) -> (Vec<rigor::AnalysisResult>, bool) {
    let mut results = Vec::new();
    let mut had_errors = false;
    let mut cache_hits = 0;

    for file in files {
        // Try to read file content for caching
        let test_content = std::fs::read_to_string(file).ok();
        
        // Check cache first
        if let Some(ref content) = test_content {
            if let Some(cached) = cache.get(file, content, None) {
                results.push(cached);
                cache_hits += 1;
                continue;
            }
        }

        // Analyze the file
        match engine.analyze(file, Some(config)) {
            Ok(result) => {
                // Store in cache
                if let Some(ref content) = test_content {
                    cache.set(file, content, None, result.clone());
                }
                results.push(result);
            }
            Err(e) => {
                if !quiet {
                    eprintln!(
                        "{}: Failed to analyze {}: {}",
                        "Error".red(),
                        file.display(),
                        e
                    );
                }
                had_errors = true;
            }
        }
    }

    if !quiet && cache_hits > 0 {
        eprintln!(
            "{}: {} files from cache, {} analyzed",
            "Cache".blue(),
            cache_hits,
            files.len() - cache_hits
        );
    }

    (results, had_errors)
}

/// Analyze files in parallel without caching
fn analyze_files_parallel(
    engine: &AnalysisEngine,
    files: &[PathBuf],
    config: &rigor::config::Config,
    quiet: bool,
) -> (Vec<rigor::AnalysisResult>, bool) {
    use rayon::prelude::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    let had_errors = AtomicBool::new(false);
    let quiet = quiet;

    let results: Vec<_> = files
        .par_iter()
        .filter_map(|file| {
            match engine.analyze(file, Some(config)) {
                Ok(result) => Some(result),
                Err(e) => {
                    had_errors.store(true, Ordering::Relaxed);
                    if !quiet {
                        eprintln!(
                            "{}: Failed to analyze {}: {}",
                            "Error".red(),
                            file.display(),
                            e
                        );
                    }
                    None
                }
            }
        })
        .collect();

    (results, had_errors.load(Ordering::Relaxed))
}

/// Analyze files in parallel with thread-safe caching
fn analyze_files_parallel_cached(
    engine: &AnalysisEngine,
    files: &[PathBuf],
    config: &rigor::config::Config,
    cache: &AnalysisCache,
    quiet: bool,
) -> (Vec<rigor::AnalysisResult>, bool) {
    use rayon::prelude::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    let had_errors = AtomicBool::new(false);
    let cache_hits = AtomicUsize::new(0);

    let results: Vec<_> = files
        .par_iter()
        .filter_map(|file| {
            // Try cache first
            if let Ok(content) = std::fs::read_to_string(file) {
                if let Some(cached) = cache.get(file, &content, None) {
                    cache_hits.fetch_add(1, Ordering::Relaxed);
                    return Some(cached);
                }
            }

            // Analyze the file
            match engine.analyze(file, Some(config)) {
                Ok(result) => Some(result),
                Err(e) => {
                    had_errors.store(true, Ordering::Relaxed);
                    if !quiet {
                        eprintln!(
                            "{}: Failed to analyze {}: {}",
                            "Error".red(),
                            file.display(),
                            e
                        );
                    }
                    None
                }
            }
        })
        .collect();

    let hits = cache_hits.load(Ordering::Relaxed);
    if !quiet && hits > 0 {
        eprintln!(
            "{}: {} files from cache, {} analyzed",
            "Cache".blue(),
            hits,
            files.len() - hits
        );
    }

    (results, had_errors.load(Ordering::Relaxed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_test_file() {
        let default_patterns = [
            ".test.ts",
            ".test.tsx",
            ".spec.ts",
            ".spec.tsx",
            ".test.js",
            ".test.jsx",
            ".spec.js",
            ".spec.jsx",
        ];
        assert!(is_test_file(std::path::Path::new("foo.test.ts"), &default_patterns));
        assert!(is_test_file(std::path::Path::new("bar.spec.tsx"), &default_patterns));
        assert!(!is_test_file(std::path::Path::new("util.ts"), &default_patterns));
        assert!(!is_test_file(std::path::Path::new("node_modules/foo.test.ts"), &default_patterns));
    }

    #[test]
    fn test_is_test_file_custom_patterns() {
        let custom_patterns = [".integration.ts", "_test.ts"];
        assert!(is_test_file(std::path::Path::new("auth.integration.ts"), &custom_patterns));
        assert!(is_test_file(std::path::Path::new("user_test.ts"), &custom_patterns));
        assert!(!is_test_file(std::path::Path::new("foo.test.ts"), &custom_patterns));
    }
}
