//! Source file mapping - finds source files corresponding to test files

use crate::config::{SourceMappingConfig, SourceMappingMode};
use std::path::{Path, PathBuf};

/// Maps test files to their corresponding source files
pub struct SourceMapper {
    config: SourceMappingConfig,
    project_root: Option<PathBuf>,
}

impl Default for SourceMapper {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceMapper {
    /// Create a new source mapper with default config
    pub fn new() -> Self {
        Self {
            config: SourceMappingConfig::default(),
            project_root: None,
        }
    }

    /// Create a source mapper with custom config
    pub fn with_config(config: SourceMappingConfig) -> Self {
        Self {
            config,
            project_root: None,
        }
    }

    /// Set the project root directory
    pub fn with_project_root(mut self, root: PathBuf) -> Self {
        self.project_root = Some(root);
        self
    }

    /// Find the source file for a test file (legacy static method for compatibility)
    pub fn find_source(test_path: &Path) -> Option<PathBuf> {
        Self::new().find_source_file(test_path)
    }

    /// Find the source file for a test file using config
    pub fn find_source_file(&self, test_path: &Path) -> Option<PathBuf> {
        // Check mode
        if self.config.mode == SourceMappingMode::Off {
            return None;
        }

        let test_name = test_path.file_name()?.to_str()?;
        let source_name = Self::strip_test_suffix(test_name)?;

        // Try explicit mappings first (if any configured)
        if !self.config.mappings.is_empty() {
            if let Some(result) = self.try_explicit_mappings(test_path, &source_name) {
                return Some(result);
            }
        }

        // If manual mode and no explicit mapping found, return None
        if self.config.mode == SourceMappingMode::Manual {
            return None;
        }

        // Try tsconfig paths if configured
        if self.config.mode == SourceMappingMode::Tsconfig {
            if let Some(result) = self.try_tsconfig_paths(test_path, &source_name) {
                return Some(result);
            }
        }

        // Use configured source/test roots if available
        if self.config.source_root.is_some() || self.config.test_root.is_some() {
            if let Some(result) = self.try_configured_roots(test_path, &source_name) {
                return Some(result);
            }
        }

        // Fall back to auto-detection strategies
        self.try_auto_strategies(test_path, &source_name)
    }

    /// Try explicit glob-based mappings from config
    fn try_explicit_mappings(&self, test_path: &Path, source_name: &str) -> Option<PathBuf> {
        let test_path_str = test_path.to_string_lossy();

        for (test_pattern, source_pattern) in &self.config.mappings {
            // Simple glob matching
            if Self::matches_glob(&test_path_str, test_pattern) {
                // Transform the path according to the mapping
                if let Some(source_path) =
                    self.transform_path(test_path, source_name, test_pattern, source_pattern)
                {
                    if source_path.exists() {
                        return Some(source_path);
                    }
                }
            }
        }
        None
    }

    /// Simple glob matching (supports ** and *)
    fn matches_glob(path: &str, pattern: &str) -> bool {
        if let Ok(glob) = globset::Glob::new(pattern) {
            let matcher = glob.compile_matcher();
            return matcher.is_match(path);
        }
        // Fallback: simple contains check for the non-glob part
        let simplified = pattern
            .replace("**", "")
            .replace('*', "")
            .replace("//", "/");
        path.contains(&simplified)
    }

    /// Transform test path to source path based on mapping patterns
    fn transform_path(
        &self,
        test_path: &Path,
        source_name: &str,
        test_pattern: &str,
        source_pattern: &str,
    ) -> Option<PathBuf> {
        let project_root = if let Some(ref root) = self.project_root {
            root.clone()
        } else {
            test_path.ancestors().nth(3)?.to_path_buf()
        };

        // Extract the relative structure from the test pattern
        // e.g., "tests/**/*.test.ts" -> "src/**/*.ts"
        // test_path: /project/tests/auth/login.test.ts
        // result: /project/src/auth/login.ts

        let test_path_str = test_path.to_string_lossy();

        // Find where the pattern starts matching
        let test_base = test_pattern
            .split("**")
            .next()
            .unwrap_or("")
            .trim_end_matches('/');
        let source_base = source_pattern
            .split("**")
            .next()
            .unwrap_or("")
            .trim_end_matches('/');

        if let Some(idx) = test_path_str.find(test_base) {
            let prefix = &test_path_str[..idx];
            let relative = test_path_str[idx + test_base.len()..]
                .trim_start_matches('/')
                .to_string();

            // Replace test directory with source directory
            let source_dir = if source_base.is_empty() {
                PathBuf::from(prefix)
            } else {
                PathBuf::from(format!("{}{}", prefix, source_base))
            };

            // Reconstruct path with source name
            let relative_dir = Path::new(&relative).parent()?;
            let source_path = source_dir.join(relative_dir);

            return Self::find_in_dir(&source_path, source_name);
        }

        // Fallback: try direct substitution at project root
        let source_dir = project_root.join(source_base.trim_start_matches('/'));
        if let Ok(relative) = test_path.strip_prefix(&project_root) {
            let relative_dir = relative.parent()?;
            // Try to find in source_dir with same relative structure
            let candidate_dir = source_dir.join(
                relative_dir
                    .to_string_lossy()
                    .replace(test_base.trim_start_matches('/'), ""),
            );
            return Self::find_in_dir(&candidate_dir, source_name);
        }

        None
    }

    /// Try to find source using tsconfig.json paths
    fn try_tsconfig_paths(&self, test_path: &Path, source_name: &str) -> Option<PathBuf> {
        // Find tsconfig.json
        let tsconfig_path = self.find_tsconfig(test_path)?;
        let tsconfig_dir = tsconfig_path.parent()?;

        // Read and parse tsconfig
        let content = std::fs::read_to_string(&tsconfig_path).ok()?;
        let tsconfig: serde_json::Value = serde_json::from_str(&content).ok()?;

        // Get compilerOptions.paths and baseUrl
        let compiler_options = tsconfig.get("compilerOptions")?;
        let base_url = compiler_options
            .get("baseUrl")
            .and_then(|v| v.as_str())
            .unwrap_or(".");
        let paths = compiler_options.get("paths")?.as_object()?;

        let base_dir = tsconfig_dir.join(base_url);

        // Try each path alias
        for (_alias, targets) in paths {
            let targets = targets.as_array()?;
            for target in targets {
                let target_pattern = target.as_str()?;
                let target_dir = target_pattern.trim_end_matches('*');
                let candidate_dir = base_dir.join(target_dir);

                if let Some(result) = Self::find_in_dir(&candidate_dir, source_name) {
                    return Some(result);
                }

                // Also try subdirectories matching the test file's relative path
                if let Some(relative) = self.get_relative_path(test_path) {
                    let candidate_dir = candidate_dir.join(relative.parent()?);
                    if let Some(result) = Self::find_in_dir(&candidate_dir, source_name) {
                        return Some(result);
                    }
                }
            }
        }

        None
    }

    /// Find tsconfig.json
    fn find_tsconfig(&self, start_path: &Path) -> Option<PathBuf> {
        let mut dir = start_path.parent()?;
        loop {
            let tsconfig = dir.join("tsconfig.json");
            if tsconfig.exists() {
                return Some(tsconfig);
            }
            dir = dir.parent()?;
        }
    }

    /// Get relative path from project root
    fn get_relative_path(&self, path: &Path) -> Option<PathBuf> {
        if let Some(ref root) = self.project_root {
            path.strip_prefix(root).ok().map(PathBuf::from)
        } else {
            None
        }
    }

    /// Try configured source/test roots
    fn try_configured_roots(&self, test_path: &Path, source_name: &str) -> Option<PathBuf> {
        let project_root = self.find_project_root(test_path)?;

        let source_root = self
            .config
            .source_root
            .as_ref()
            .map(|r| project_root.join(r))
            .unwrap_or_else(|| project_root.join("src"));

        let test_root = self.config.test_root.as_ref().map(|r| project_root.join(r));

        // Calculate relative path from test root
        let relative = if let Some(ref test_root) = test_root {
            test_path.strip_prefix(test_root).ok()
        } else {
            // Try common test directories
            for test_dir in &["tests", "__tests__", "test", "spec"] {
                let test_dir_path = project_root.join(test_dir);
                if let Ok(rel) = test_path.strip_prefix(&test_dir_path) {
                    return Self::find_in_dir(&source_root.join(rel.parent()?), source_name);
                }
            }
            None
        };

        if let Some(relative) = relative {
            return Self::find_in_dir(&source_root.join(relative.parent()?), source_name);
        }

        None
    }

    /// Find project root from a file path
    fn find_project_root(&self, start: &Path) -> Option<PathBuf> {
        if let Some(ref root) = self.project_root {
            return Some(root.clone());
        }

        let mut dir = start.parent()?;
        loop {
            if dir.join("package.json").exists()
                || dir.join(".git").exists()
                || dir.join(".rigorrc.json").exists()
            {
                return Some(dir.to_path_buf());
            }
            dir = dir.parent()?;
        }
    }

    /// Auto-detection strategies for finding source files
    #[allow(clippy::type_complexity)]
    fn try_auto_strategies(&self, test_path: &Path, source_name: &str) -> Option<PathBuf> {
        let strategies: Vec<Box<dyn Fn(&Path, &str, &SourceMapper) -> Option<PathBuf>>> = vec![
            // 1. Adjacent file in same directory
            Box::new(|path, name, _| {
                let dir = path.parent()?;
                Self::find_in_dir(dir, name)
            }),
            // 2. __tests__ folder -> parent directory
            Box::new(|path, name, _| {
                let path_str = path.to_str()?;
                if path_str.contains("__tests__") {
                    let parent_dir = path
                        .ancestors()
                        .find(|p| p.file_name().map(|n| n == "__tests__").unwrap_or(false))?
                        .parent()?;
                    Self::find_in_dir(parent_dir, name)
                } else {
                    None
                }
            }),
            // 3. tests/ -> src/ parallel structure
            Box::new(|path, name, _| {
                let path_str = path.to_str()?;
                if path_str.contains("/tests/") {
                    let src_path = path_str.replace("/tests/", "/src/");
                    let dir = Path::new(&src_path).parent()?;
                    Self::find_in_dir(dir, name)
                } else {
                    None
                }
            }),
            // 4. test/ -> src/ parallel structure
            Box::new(|path, name, _| {
                let path_str = path.to_str()?;
                if path_str.contains("/test/") {
                    let src_path = path_str.replace("/test/", "/src/");
                    let dir = Path::new(&src_path).parent()?;
                    Self::find_in_dir(dir, name)
                } else {
                    None
                }
            }),
            // 5. spec/ -> src/ parallel structure
            Box::new(|path, name, _| {
                let path_str = path.to_str()?;
                if path_str.contains("/spec/") {
                    let src_path = path_str.replace("/spec/", "/src/");
                    let dir = Path::new(&src_path).parent()?;
                    Self::find_in_dir(dir, name)
                } else {
                    None
                }
            }),
            // 6. Parent directory
            Box::new(|path, name, _| {
                let dir = path.parent()?.parent()?;
                Self::find_in_dir(dir, name)
            }),
            // 7. src/ in parent
            Box::new(|path, name, _| {
                let parent = path.parent()?.parent()?;
                let src_dir = parent.join("src");
                Self::find_in_dir(&src_dir, name)
            }),
            // 8. lib/ parallel structure
            Box::new(|path, name, _| {
                let path_str = path.to_str()?;
                for test_dir in &["/tests/", "/test/", "/__tests__/"] {
                    if path_str.contains(test_dir) {
                        let lib_path = path_str.replace(test_dir, "/lib/");
                        let dir = Path::new(&lib_path).parent()?;
                        if let Some(result) = Self::find_in_dir(dir, name) {
                            return Some(result);
                        }
                    }
                }
                None
            }),
            // 9. Monorepo: packages/*/tests -> packages/*/src
            Box::new(|path, name, mapper| {
                let project_root = mapper.find_project_root(path)?;
                let relative = path.strip_prefix(&project_root).ok()?;
                let components: Vec<_> = relative.components().collect();

                // Look for patterns like packages/foo/tests/bar.test.ts
                for (i, component) in components.iter().enumerate() {
                    let component_str = component.as_os_str().to_str()?;
                    if component_str == "tests"
                        || component_str == "__tests__"
                        || component_str == "test"
                    {
                        // Reconstruct path with src instead of tests
                        let mut new_path = project_root.clone();
                        for c in components.iter().take(i) {
                            new_path = new_path.join(c.as_os_str());
                        }
                        new_path = new_path.join("src");
                        for c in components.iter().take(components.len() - 1).skip(i + 1) {
                            new_path = new_path.join(c.as_os_str());
                        }
                        if let Some(result) = Self::find_in_dir(&new_path, name) {
                            return Some(result);
                        }
                    }
                }
                None
            }),
        ];

        for strategy in strategies {
            if let Some(source) = strategy(test_path, source_name, self) {
                return Some(source);
            }
        }

        None
    }

    /// Strip test suffixes to get the base source name
    pub fn strip_test_suffix(test_name: &str) -> Option<String> {
        let patterns = [
            ".test.ts",
            ".test.tsx",
            ".spec.ts",
            ".spec.tsx",
            ".cy.ts",
            ".cy.tsx",
            ".test.js",
            ".test.jsx",
            ".spec.js",
            ".spec.jsx",
            ".cy.js",
            ".cy.jsx",
            "_test.ts",
            "_test.tsx",
            "_spec.ts",
            "_spec.tsx",
            ".test.mts",
            ".spec.mts",
            ".test.mjs",
            ".spec.mjs",
        ];

        for pattern in patterns {
            if let Some(base) = test_name.strip_suffix(pattern) {
                return Some(base.to_string());
            }
        }

        None
    }

    /// Find a source file by name in a directory
    pub fn find_in_dir(dir: &Path, base_name: &str) -> Option<PathBuf> {
        if !dir.exists() {
            return None;
        }

        let extensions = ["ts", "tsx", "js", "jsx", "mts", "mjs"];

        for ext in extensions {
            let candidate = dir.join(format!("{}.{}", base_name, ext));
            if candidate.exists() {
                return Some(candidate);
            }
        }

        // Also check for index file in a subdirectory with the same name
        let subdir = dir.join(base_name);
        if subdir.is_dir() {
            for ext in extensions {
                let index = subdir.join(format!("index.{}", ext));
                if index.exists() {
                    return Some(index);
                }
            }
        }

        None
    }

    /// Check if a file is likely a test utility/helper (not a real test file)
    pub fn is_test_utility(path: &Path) -> bool {
        let file_stem = path.file_stem().and_then(|n| n.to_str()).unwrap_or("");

        // Common test utility file name patterns
        let utility_file_patterns = [
            "testUtils",
            "test-utils",
            "testHelpers",
            "test-helpers",
            "setup",
            "setupTests",
            "jest.setup",
            "vitest.setup",
            "globalSetup",
            "globalTeardown",
            "factory",
            "factories",
            "builder",
            "builders",
        ];

        let stem_lower = file_stem.to_lowercase();
        for pattern in utility_file_patterns {
            if stem_lower.contains(&pattern.to_lowercase()) {
                return true;
            }
        }

        // Check if in a utility directory (check path components)
        let path_str = path.to_string_lossy().to_lowercase();
        let utility_dirs = [
            "__mocks__",
            "__fixtures__",
            "test-utils",
            "test-helpers",
            "fixtures",
            "mocks",
            "/helpers/",
        ];
        for dir in utility_dirs {
            if path_str.contains(dir) {
                return true;
            }
        }

        // Check path components for utility directories
        for component in path.components() {
            if let Some(name) = component.as_os_str().to_str() {
                let name_lower = name.to_lowercase();
                if name_lower == "fixtures"
                    || name_lower == "mocks"
                    || name_lower == "__mocks__"
                    || name_lower == "__fixtures__"
                    || name_lower == "helpers"
                    || name_lower == "test-utils"
                    || name_lower == "test-helpers"
                {
                    return true;
                }
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_test_suffix() {
        assert_eq!(
            SourceMapper::strip_test_suffix("auth.test.ts"),
            Some("auth".to_string())
        );
        assert_eq!(
            SourceMapper::strip_test_suffix("auth.spec.tsx"),
            Some("auth".to_string())
        );
        assert_eq!(
            SourceMapper::strip_test_suffix("utils_test.ts"),
            Some("utils".to_string())
        );
        assert_eq!(SourceMapper::strip_test_suffix("regular.ts"), None);
        assert_eq!(
            SourceMapper::strip_test_suffix("Button.test.mts"),
            Some("Button".to_string())
        );
        assert_eq!(
            SourceMapper::strip_test_suffix("conversations.cy.ts"),
            Some("conversations".to_string())
        );
    }

    #[test]
    fn test_is_test_utility() {
        assert!(SourceMapper::is_test_utility(Path::new("src/testUtils.ts")));
        assert!(SourceMapper::is_test_utility(Path::new(
            "src/__mocks__/api.ts"
        )));
        assert!(SourceMapper::is_test_utility(Path::new(
            "tests/fixtures/user.ts"
        )));
        assert!(SourceMapper::is_test_utility(Path::new("tests/setup.ts")));
        assert!(!SourceMapper::is_test_utility(Path::new(
            "src/auth.test.ts"
        )));
    }

    #[test]
    fn test_matches_glob() {
        assert!(SourceMapper::matches_glob(
            "tests/auth/login.test.ts",
            "tests/**/*.test.ts"
        ));
        assert!(SourceMapper::matches_glob(
            "src/__tests__/auth.test.ts",
            "**/__tests__/**"
        ));
        assert!(!SourceMapper::matches_glob(
            "src/auth.ts",
            "tests/**/*.test.ts"
        ));
    }
}
