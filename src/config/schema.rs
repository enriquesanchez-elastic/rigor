//! Config schema and deserialization

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Rule severity override (error, warning, info, off)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleSeverity {
    Error,
    Warning,
    Info,
    /// Disable the rule entirely
    Off,
}

impl RuleSeverity {
    /// Convert to crate::Severity if not Off
    pub fn to_severity(self) -> Option<crate::Severity> {
        match self {
            RuleSeverity::Error => Some(crate::Severity::Error),
            RuleSeverity::Warning => Some(crate::Severity::Warning),
            RuleSeverity::Info => Some(crate::Severity::Info),
            RuleSeverity::Off => None,
        }
    }
}

/// Framework override: auto-detect or force a specific framework
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FrameworkOverride {
    #[default]
    Auto,
    Jest,
    Vitest,
    Playwright,
    Cypress,
    Mocha,
}

/// Source mapping mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SourceMappingMode {
    /// Auto-detect using common patterns (default)
    #[default]
    Auto,
    /// Use tsconfig.json paths for resolution
    Tsconfig,
    /// Only use explicit mappings from config
    Manual,
    /// Disable source file analysis
    Off,
}

/// Source mapping configuration
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SourceMappingConfig {
    /// Source mapping mode
    #[serde(default)]
    pub mode: SourceMappingMode,

    /// Explicit glob-based mappings: test pattern -> source pattern
    /// e.g., "tests/**/*.test.ts" -> "src/**/*.ts"
    #[serde(default)]
    pub mappings: HashMap<String, String>,

    /// Root directory for source files (relative to project root)
    #[serde(default)]
    pub source_root: Option<String>,

    /// Root directory for test files (relative to project root)
    #[serde(default)]
    pub test_root: Option<String>,
}

/// Per-path override configuration
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigOverride {
    /// Glob patterns this override applies to
    pub files: Vec<String>,

    /// Optional threshold override for matched files
    #[serde(default)]
    pub threshold: Option<u8>,

    /// Optional rule overrides for matched files
    #[serde(default)]
    pub rules: HashMap<String, RuleSeverity>,

    /// Skip source analysis for matched files (e.g., E2E tests)
    #[serde(default)]
    pub skip_source_analysis: Option<bool>,
}

/// Root config structure for .rigorrc.json
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    /// Extend another config file (path relative to this config, or package name)
    #[serde(default)]
    pub extends: Option<String>,

    /// Minimum score threshold (exit 1 if below). Default: 0
    #[serde(default)]
    pub threshold: Option<u8>,

    /// Per-rule severity overrides. Key is rule name in kebab-case.
    #[serde(default)]
    pub rules: HashMap<String, RuleSeverity>,

    /// Glob patterns for files/directories to exclude from analysis
    #[serde(default)]
    pub ignore: Vec<String>,

    /// Force a specific test framework (default: auto)
    #[serde(default)]
    pub framework: FrameworkOverride,

    /// Source file mapping configuration
    #[serde(default)]
    pub source_mapping: SourceMappingConfig,

    /// Custom test file patterns (default: *.test.ts, *.spec.ts, etc.)
    #[serde(default)]
    pub test_patterns: Vec<String>,

    /// Root directory to search for test files recursively (relative to project root)
    /// If not set, searches from the path provided on the command line
    #[serde(default)]
    pub test_root: Option<String>,

    /// Per-path configuration overrides (for monorepos, legacy code, etc.)
    #[serde(default)]
    pub overrides: Vec<ConfigOverride>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            extends: None,
            threshold: None,
            rules: HashMap::new(),
            ignore: Vec::new(),
            framework: FrameworkOverride::Auto,
            source_mapping: SourceMappingConfig::default(),
            test_patterns: Vec::new(),
            test_root: None,
            overrides: Vec::new(),
        }
    }
}

impl Config {
    /// Merge CLI overrides into config. CLI values take precedence.
    pub fn merge_with_cli(
        mut self,
        cli_threshold: Option<u8>,
        cli_config_path: Option<&Path>,
    ) -> Self {
        if cli_threshold.is_some() {
            self.threshold = cli_threshold;
        }
        // cli_config_path is used for loading, not stored in config
        let _ = cli_config_path;
        self
    }

    /// Get the effective severity for a rule, or None if rule is off / not in config
    pub fn rule_severity(&self, rule_id: &str) -> Option<RuleSeverity> {
        self.rules.get(rule_id).copied()
    }

    /// Get effective config for a specific file path, applying overrides
    pub fn effective_for_file(&self, file_path: &Path) -> EffectiveConfig {
        let mut effective = EffectiveConfig {
            threshold: self.threshold,
            rules: self.rules.clone(),
            skip_source_analysis: false,
        };

        // Apply matching overrides in order
        for override_cfg in &self.overrides {
            if Self::matches_override(file_path, &override_cfg.files) {
                if let Some(threshold) = override_cfg.threshold {
                    effective.threshold = Some(threshold);
                }
                for (rule, severity) in &override_cfg.rules {
                    effective.rules.insert(rule.clone(), *severity);
                }
                if let Some(skip) = override_cfg.skip_source_analysis {
                    effective.skip_source_analysis = skip;
                }
            }
        }

        effective
    }

    /// Check if a file path matches any of the override patterns
    fn matches_override(file_path: &Path, patterns: &[String]) -> bool {
        let path_str = file_path.to_string_lossy();
        for pattern in patterns {
            if let Ok(glob) = globset::Glob::new(pattern) {
                let matcher = glob.compile_matcher();
                if matcher.is_match(file_path)
                    || path_str.contains(pattern.trim_start_matches("**/"))
                {
                    return true;
                }
            }
        }
        false
    }

    /// Merge another config into this one (for extends)
    pub fn merge_from(&mut self, base: Config) {
        // Base values are overridden by this config's values
        if self.threshold.is_none() {
            self.threshold = base.threshold;
        }
        if self.extends.is_none() {
            self.extends = base.extends;
        }
        if self.framework == FrameworkOverride::Auto {
            self.framework = base.framework;
        }

        // Merge rules (this config takes precedence)
        for (rule, severity) in base.rules {
            self.rules.entry(rule).or_insert(severity);
        }

        // Merge ignore patterns
        let mut all_ignores = base.ignore;
        all_ignores.append(&mut self.ignore);
        self.ignore = all_ignores;

        // Merge test patterns
        if self.test_patterns.is_empty() {
            self.test_patterns = base.test_patterns;
        }

        // Merge test root
        if self.test_root.is_none() {
            self.test_root = base.test_root;
        }

        // Merge source mapping (this config takes precedence for non-default values)
        if self.source_mapping.mode == SourceMappingMode::Auto {
            self.source_mapping.mode = base.source_mapping.mode;
        }
        if self.source_mapping.source_root.is_none() {
            self.source_mapping.source_root = base.source_mapping.source_root;
        }
        if self.source_mapping.test_root.is_none() {
            self.source_mapping.test_root = base.source_mapping.test_root;
        }
        for (pattern, target) in base.source_mapping.mappings {
            self.source_mapping
                .mappings
                .entry(pattern)
                .or_insert(target);
        }

        // Prepend base overrides
        let mut all_overrides = base.overrides;
        all_overrides.append(&mut self.overrides);
        self.overrides = all_overrides;
    }

    /// Get default test file patterns
    pub fn get_test_patterns(&self) -> Vec<&str> {
        if self.test_patterns.is_empty() {
            vec![
                ".test.ts",
                ".test.tsx",
                ".spec.ts",
                ".spec.tsx",
                ".test.js",
                ".test.jsx",
                ".spec.js",
                ".spec.jsx",
                // Cypress test files
                ".cy.ts",
                ".cy.tsx",
                ".cy.js",
                ".cy.jsx",
            ]
        } else {
            self.test_patterns.iter().map(|s| s.as_str()).collect()
        }
    }
}

/// Effective configuration for a specific file (after applying overrides)
#[derive(Debug, Clone)]
pub struct EffectiveConfig {
    pub threshold: Option<u8>,
    pub rules: HashMap<String, RuleSeverity>,
    pub skip_source_analysis: bool,
}
