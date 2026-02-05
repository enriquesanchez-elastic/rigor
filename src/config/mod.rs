//! Configuration loading for Rigor

mod schema;

pub use schema::{
    Config, ConfigOverride, EffectiveConfig, FrameworkOverride, RuleSeverity, SourceMappingConfig,
    SourceMappingMode,
};

use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub const CONFIG_FILENAME: &str = ".rigorrc.json";

/// Find and load config file with extends resolution. Searches current directory then parents.
pub fn load_config(work_dir: &Path, custom_path: Option<&Path>) -> Result<Config> {
    let path = if let Some(p) = custom_path {
        let path = if p.is_absolute() {
            p.to_path_buf()
        } else {
            work_dir.join(p)
        };
        if path.exists() {
            Some(path)
        } else {
            anyhow::bail!("Config file not found: {}", path.display());
        }
    } else {
        find_config_in_parents(work_dir)?
    };

    match path {
        Some(path) => load_config_with_extends(&path, &mut HashSet::new()),
        None => Ok(Config::default()),
    }
}

/// Load a config file and resolve extends chain
fn load_config_with_extends(config_path: &Path, visited: &mut HashSet<PathBuf>) -> Result<Config> {
    // Prevent circular extends
    let canonical = config_path
        .canonicalize()
        .unwrap_or_else(|_| config_path.to_path_buf());
    if visited.contains(&canonical) {
        anyhow::bail!(
            "Circular extends detected in config: {}",
            config_path.display()
        );
    }
    visited.insert(canonical.clone());

    let content = fs::read_to_string(config_path)
        .with_context(|| format!("Failed to read config: {}", config_path.display()))?;
    let mut config: Config = serde_json::from_str(&content)
        .with_context(|| format!("Invalid JSON in config: {}", config_path.display()))?;

    // Resolve extends
    if let Some(extends) = config.extends.take() {
        let base_config = resolve_extends(config_path, &extends, visited)?;
        config.merge_from(base_config);
    }

    Ok(config)
}

/// Resolve an extends reference to a config
fn resolve_extends(
    config_path: &Path,
    extends: &str,
    visited: &mut HashSet<PathBuf>,
) -> Result<Config> {
    let config_dir = config_path.parent().unwrap_or(Path::new("."));

    // Try as relative path first
    let extends_path = if extends.starts_with("./") || extends.starts_with("../") {
        config_dir.join(extends)
    } else if extends.starts_with('/') {
        PathBuf::from(extends)
    } else {
        // Could be a package reference like "@company/rigor-config"
        // Try to find it in node_modules
        let node_modules_path = find_node_modules_config(config_dir, extends);
        if let Some(path) = node_modules_path {
            path
        } else {
            // Fall back to treating as relative path
            config_dir.join(extends)
        }
    };

    // Ensure it has .json extension
    let extends_path = if extends_path.extension().is_none() {
        extends_path.with_extension("json")
    } else {
        extends_path
    };

    if !extends_path.exists() {
        anyhow::bail!(
            "Extended config not found: {} (referenced from {})",
            extends_path.display(),
            config_path.display()
        );
    }

    load_config_with_extends(&extends_path, visited)
}

/// Try to find a config in node_modules
fn find_node_modules_config(start_dir: &Path, package: &str) -> Option<PathBuf> {
    let mut dir = start_dir;
    loop {
        let node_modules = dir.join("node_modules").join(package);

        // Try common config file locations in the package
        for filename in &[".rigorrc.json", "rigor.config.json", "index.json"] {
            let candidate = node_modules.join(filename);
            if candidate.exists() {
                return Some(candidate);
            }
        }

        // Also check if it's directly a .json file reference
        let direct = dir.join("node_modules").join(format!("{}.json", package));
        if direct.exists() {
            return Some(direct);
        }

        dir = dir.parent()?;
    }
}

/// Search for .rigorrc.json in directory and its parents
fn find_config_in_parents(mut dir: &Path) -> Result<Option<PathBuf>> {
    loop {
        let candidate = dir.join(CONFIG_FILENAME);
        if candidate.exists() {
            return Ok(Some(candidate));
        }
        dir = match dir.parent() {
            Some(p) => p,
            None => return Ok(None),
        };
    }
}

/// Build a GlobSet from ignore patterns for path matching
pub fn build_ignore_set(patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob =
            Glob::new(pattern).with_context(|| format!("Invalid ignore pattern: {}", pattern))?;
        builder.add(glob);
    }
    builder.build().map_err(|e| anyhow::anyhow!("{}", e))
}

/// Check if a path should be ignored based on config glob patterns
pub fn is_ignored(path: &Path, ignore_set: &GlobSet) -> bool {
    ignore_set.is_match(path)
}

/// Find the project root directory (containing package.json, .git, or config file)
pub fn find_project_root(start_dir: &Path) -> Option<PathBuf> {
    let mut dir = start_dir;
    loop {
        // Check for project markers
        if dir.join("package.json").exists()
            || dir.join(".git").exists()
            || dir.join(CONFIG_FILENAME).exists()
        {
            return Some(dir.to_path_buf());
        }
        dir = dir.parent()?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_find_config_none() {
        let dir = std::env::temp_dir();
        let result = find_config_in_parents(&dir).unwrap();
        let _ = result;
    }

    #[test]
    fn test_is_ignored_e2e() {
        let set = build_ignore_set(&["**/*.e2e.test.ts".to_string()]).unwrap();
        assert!(is_ignored(Path::new("src/auth.e2e.test.ts"), &set));
        assert!(!is_ignored(Path::new("src/auth.test.ts"), &set));
    }

    #[test]
    fn test_is_ignored_legacy() {
        let set = build_ignore_set(&["**/legacy/**".to_string()]).unwrap();
        assert!(is_ignored(Path::new("foo/legacy/bar.test.ts"), &set));
    }

    #[test]
    fn test_config_extends() {
        let dir = TempDir::new().unwrap();

        // Create base config
        let base_path = dir.path().join("base.json");
        let mut base_file = fs::File::create(&base_path).unwrap();
        writeln!(
            base_file,
            r#"{{
                "threshold": 70,
                "rules": {{ "weak-assertion": "error" }},
                "ignore": ["**/legacy/**"]
            }}"#
        )
        .unwrap();

        // Create child config that extends base
        let child_path = dir.path().join(".rigorrc.json");
        let mut child_file = fs::File::create(&child_path).unwrap();
        writeln!(
            child_file,
            r#"{{
                "extends": "./base.json",
                "threshold": 80,
                "rules": {{ "no-assertions": "error" }}
            }}"#
        )
        .unwrap();

        let config = load_config(dir.path(), None).unwrap();

        // Child threshold overrides base
        assert_eq!(config.threshold, Some(80));
        // Child rule is present
        assert!(config.rules.contains_key("no-assertions"));
        // Base rule is inherited
        assert!(config.rules.contains_key("weak-assertion"));
        // Base ignore is inherited
        assert!(config.ignore.contains(&"**/legacy/**".to_string()));
    }

    #[test]
    fn test_config_overrides() {
        let config: Config = serde_json::from_str(
            r#"{
                "threshold": 70,
                "overrides": [
                    {
                        "files": ["**/legacy/**"],
                        "threshold": 50,
                        "rules": { "weak-assertion": "off" }
                    },
                    {
                        "files": ["**/*.e2e.test.ts"],
                        "skipSourceAnalysis": true
                    }
                ]
            }"#,
        )
        .unwrap();

        // Regular file
        let effective = config.effective_for_file(Path::new("src/auth.test.ts"));
        assert_eq!(effective.threshold, Some(70));
        assert!(!effective.skip_source_analysis);

        // Legacy file
        let effective = config.effective_for_file(Path::new("src/legacy/old.test.ts"));
        assert_eq!(effective.threshold, Some(50));

        // E2E file
        let effective = config.effective_for_file(Path::new("src/auth.e2e.test.ts"));
        assert!(effective.skip_source_analysis);
    }
}
