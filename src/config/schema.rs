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

    /// Scoring algorithm version: "v1" (default) or "v2".
    /// v2: no double-counting — issues that affect a category score do not also add penalty.
    #[serde(default)]
    pub scoring_version: Option<String>,
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
            scoring_version: None,
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

        if self.scoring_version.is_none() {
            self.scoring_version = base.scoring_version;
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_severity_to_severity() {
        assert_eq!(
            RuleSeverity::Error.to_severity(),
            Some(crate::Severity::Error)
        );
        assert_eq!(
            RuleSeverity::Warning.to_severity(),
            Some(crate::Severity::Warning)
        );
        assert_eq!(
            RuleSeverity::Info.to_severity(),
            Some(crate::Severity::Info)
        );
        assert_eq!(RuleSeverity::Off.to_severity(), None);
    }

    #[test]
    fn test_rule_severity_from_config() {
        let mut rules = HashMap::new();
        rules.insert("weak-assertion".to_string(), RuleSeverity::Error);
        rules.insert("debug-code".to_string(), RuleSeverity::Off);

        let config = Config {
            rules,
            ..Config::default()
        };

        assert_eq!(
            config.rule_severity("weak-assertion"),
            Some(RuleSeverity::Error)
        );
        assert_eq!(config.rule_severity("debug-code"), Some(RuleSeverity::Off));
        assert_eq!(config.rule_severity("nonexistent-rule"), None);
    }

    #[test]
    fn test_effective_for_file_e2e_override() {
        let config: Config = serde_json::from_str(
            r#"{
                "threshold": 70,
                "overrides": [
                    {
                        "files": ["**/*.e2e.test.ts"],
                        "skipSourceAnalysis": true,
                        "threshold": 50,
                        "rules": { "weak-assertion": "off" }
                    }
                ]
            }"#,
        )
        .unwrap();

        let effective = config.effective_for_file(Path::new("src/auth.e2e.test.ts"));
        assert!(effective.skip_source_analysis);
        assert_eq!(effective.threshold, Some(50));
        assert_eq!(
            effective.rules.get("weak-assertion"),
            Some(&RuleSeverity::Off)
        );
    }

    #[test]
    fn test_effective_for_file_no_override_match() {
        let config: Config = serde_json::from_str(
            r#"{
                "threshold": 70,
                "rules": { "debug-code": "error" },
                "overrides": [
                    {
                        "files": ["**/legacy/**"],
                        "threshold": 40
                    }
                ]
            }"#,
        )
        .unwrap();

        let effective = config.effective_for_file(Path::new("src/auth.test.ts"));
        assert_eq!(effective.threshold, Some(70));
        assert!(!effective.skip_source_analysis);
        assert_eq!(
            effective.rules.get("debug-code"),
            Some(&RuleSeverity::Error)
        );
    }

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.threshold, None);
        assert!(config.rules.is_empty());
        assert!(config.ignore.is_empty());
        assert_eq!(config.framework, FrameworkOverride::Auto);
        assert_eq!(config.source_mapping.mode, SourceMappingMode::Auto);
    }

    #[test]
    fn test_config_merge_with_cli() {
        let config = Config {
            threshold: Some(70),
            ..Config::default()
        };

        let merged = config.merge_with_cli(Some(90), None);
        assert_eq!(merged.threshold, Some(90));
    }

    #[test]
    fn test_config_merge_with_cli_no_override() {
        let config = Config {
            threshold: Some(70),
            ..Config::default()
        };

        let merged = config.merge_with_cli(None, None);
        assert_eq!(merged.threshold, Some(70));
    }

    #[test]
    fn test_get_test_patterns_default() {
        let config = Config::default();
        let patterns = config.get_test_patterns();
        assert!(patterns.contains(&".test.ts"));
        assert!(patterns.contains(&".spec.ts"));
        assert!(patterns.contains(&".cy.ts"));
    }

    #[test]
    fn test_get_test_patterns_custom() {
        let config = Config {
            test_patterns: vec!["_test.ts".to_string()],
            ..Config::default()
        };
        let patterns = config.get_test_patterns();
        assert_eq!(patterns, vec!["_test.ts"]);
    }

    #[test]
    fn test_config_merge_from() {
        let mut child = Config {
            threshold: Some(80),
            ..Config::default()
        };

        let mut base_rules = HashMap::new();
        base_rules.insert("weak-assertion".to_string(), RuleSeverity::Error);
        base_rules.insert("debug-code".to_string(), RuleSeverity::Warning);

        let base = Config {
            threshold: Some(60),
            rules: base_rules,
            ignore: vec!["**/legacy/**".to_string()],
            framework: FrameworkOverride::Vitest,
            ..Config::default()
        };

        child.merge_from(base);

        // Child threshold takes precedence
        assert_eq!(child.threshold, Some(80));
        // Base rules inherited
        assert_eq!(
            child.rules.get("weak-assertion"),
            Some(&RuleSeverity::Error)
        );
        assert_eq!(child.rules.get("debug-code"), Some(&RuleSeverity::Warning));
        // Base ignore inherited
        assert!(child.ignore.contains(&"**/legacy/**".to_string()));
        // Base framework inherited when child is Auto
        assert_eq!(child.framework, FrameworkOverride::Vitest);
    }

    #[test]
    fn test_config_deserialization_full() {
        let json = r#"{
            "threshold": 75,
            "framework": "jest",
            "rules": {
                "weak-assertion": "error",
                "debug-code": "off",
                "no-assertions": "warning"
            },
            "ignore": ["**/legacy/**", "**/generated/**"],
            "sourceMapping": {
                "mode": "manual",
                "sourceRoot": "src",
                "testRoot": "tests",
                "mappings": {
                    "tests/**/*.test.ts": "src/**/*.ts"
                }
            },
            "testPatterns": [".test.ts", ".spec.ts"],
            "testRoot": "tests"
        }"#;

        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.threshold, Some(75));
        assert_eq!(config.framework, FrameworkOverride::Jest);
        assert_eq!(config.rules.len(), 3);
        assert_eq!(config.ignore.len(), 2);
        assert_eq!(config.source_mapping.mode, SourceMappingMode::Manual);
        assert_eq!(config.source_mapping.source_root, Some("src".to_string()));
        assert_eq!(config.source_mapping.mappings.len(), 1);
        assert_eq!(config.test_patterns.len(), 2);
        assert_eq!(config.test_root, Some("tests".to_string()));
    }

    #[test]
    fn test_multiple_overrides_applied_in_order() {
        let config: Config = serde_json::from_str(
            r#"{
                "threshold": 70,
                "overrides": [
                    {
                        "files": ["**/tests/**"],
                        "threshold": 60
                    },
                    {
                        "files": ["**/tests/legacy/**"],
                        "threshold": 40
                    }
                ]
            }"#,
        )
        .unwrap();

        // A file matching both overrides: second should win (applied in order)
        let effective = config.effective_for_file(Path::new("src/tests/legacy/old.test.ts"));
        assert_eq!(effective.threshold, Some(40));
    }
}
