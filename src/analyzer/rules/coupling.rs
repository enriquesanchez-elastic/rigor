//! Test-source coupling analysis rule
//!
//! Detects:
//! - Untested exports: source file exports that aren't referenced in tests
//! - Dead imports: test file imports that aren't used in assertions

use super::AnalysisRule;
use crate::{Issue, Location, Rule, Severity, TestCase};
use std::collections::HashSet;
use tree_sitter::{Node, Tree};

/// Rule for analyzing test-source coupling
pub struct CouplingAnalysisRule {
    /// Exports from source file that should be tested
    source_exports: Vec<String>,
}

impl CouplingAnalysisRule {
    pub fn new() -> Self {
        Self {
            source_exports: Vec::new(),
        }
    }

    /// Set source exports to check against
    pub fn with_source_exports(mut self, exports: Vec<String>) -> Self {
        self.source_exports = exports;
        self
    }

    /// Extract imports from test file
    fn extract_test_imports(source: &str, tree: &Tree) -> Vec<ImportInfo> {
        let mut imports = Vec::new();
        Self::visit_for_imports(tree.root_node(), source, &mut imports);
        imports
    }

    fn visit_for_imports(node: Node, source: &str, imports: &mut Vec<ImportInfo>) {
        if node.kind() == "import_statement" {
            if let Some(import_info) = Self::parse_import(node, source) {
                imports.push(import_info);
            }
        }

        for child in node.named_children(&mut node.walk()) {
            Self::visit_for_imports(child, source, imports);
        }
    }

    fn parse_import(node: Node, source: &str) -> Option<ImportInfo> {
        let mut imported_names = Vec::new();
        let mut source_path = String::new();
        let location = Location::new(
            node.start_position().row + 1,
            node.start_position().column + 1,
        );

        for child in node.named_children(&mut node.walk()) {
            match child.kind() {
                "import_clause" => {
                    // Handle named imports { foo, bar }
                    Self::collect_import_names(child, source, &mut imported_names);
                }
                "string" => {
                    // Import source path
                    let text = child.utf8_text(source.as_bytes()).unwrap_or("");
                    source_path = text.trim_matches(|c| c == '"' || c == '\'').to_string();
                }
                _ => {}
            }
        }

        if imported_names.is_empty() && source_path.is_empty() {
            return None;
        }

        Some(ImportInfo {
            names: imported_names,
            source: source_path,
            location,
        })
    }

    fn collect_import_names(node: Node, source: &str, names: &mut Vec<String>) {
        match node.kind() {
            "identifier" => {
                let text = node.utf8_text(source.as_bytes()).unwrap_or("");
                if !text.is_empty() {
                    names.push(text.to_string());
                }
            }
            "import_specifier" => {
                // { foo as bar } - get the local name
                if let Some(name_node) = node.child_by_field_name("name") {
                    let text = name_node.utf8_text(source.as_bytes()).unwrap_or("");
                    if !text.is_empty() {
                        names.push(text.to_string());
                    }
                }
            }
            "named_imports" => {
                for child in node.named_children(&mut node.walk()) {
                    Self::collect_import_names(child, source, names);
                }
            }
            _ => {
                for child in node.named_children(&mut node.walk()) {
                    Self::collect_import_names(child, source, names);
                }
            }
        }
    }

    /// Find which imported names are actually used in the test file
    fn find_used_imports(source: &str, imports: &[ImportInfo]) -> HashSet<String> {
        let source_lower = source.to_lowercase();
        let mut used = HashSet::new();

        for import in imports {
            for name in &import.names {
                // Simple heuristic: if the name appears elsewhere in the file, it's used
                // This is imperfect but catches most cases
                let name_lower = name.to_lowercase();

                // Count occurrences - if more than in import statement, it's used
                let count = source_lower.matches(&name_lower).count();
                if count > 1 {
                    used.insert(name.clone());
                }
            }
        }

        used
    }
}

impl Default for CouplingAnalysisRule {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
struct ImportInfo {
    names: Vec<String>,
    source: String,
    location: Location,
}

impl AnalysisRule for CouplingAnalysisRule {
    fn name(&self) -> &'static str {
        "coupling"
    }

    fn analyze(&self, tests: &[TestCase], source: &str, tree: &Tree) -> Vec<Issue> {
        let mut issues = Vec::new();

        if tests.is_empty() {
            return issues;
        }

        // Check for untested exports
        if !self.source_exports.is_empty() {
            let source_lower = source.to_lowercase();

            for export_name in &self.source_exports {
                let name_lower = export_name.to_lowercase();

                // Check if export is referenced in test file
                if !source_lower.contains(&name_lower) {
                    issues.push(Issue {
                        rule: Rule::LimitedInputVariety, // Reuse existing rule for now
                        severity: Severity::Info,
                        message: format!(
                            "Source export '{}' is not referenced in tests",
                            export_name
                        ),
                        location: Location::new(1, 1),
                        suggestion: Some(format!("Consider adding tests for '{}'", export_name)),
                    });
                }
            }
        }

        // Check for dead imports (imports not used in tests)
        let imports = Self::extract_test_imports(source, tree);
        let used_imports = Self::find_used_imports(source, &imports);

        for import in &imports {
            // Skip imports from testing libraries
            if import.source.contains("@testing-library")
                || import.source.contains("vitest")
                || import.source.contains("jest")
                || import.source.contains("cypress")
            {
                continue;
            }

            for name in &import.names {
                if !used_imports.contains(name) {
                    issues.push(Issue {
                        rule: Rule::LimitedInputVariety, // Reuse existing rule
                        severity: Severity::Info,
                        message: format!(
                            "Import '{}' from '{}' appears unused in tests",
                            name, import.source
                        ),
                        location: import.location.clone(),
                        suggestion: Some(format!(
                            "Either remove unused import or add tests that use '{}'",
                            name
                        )),
                    });
                }
            }
        }

        issues
    }

    fn calculate_score(&self, _tests: &[TestCase], issues: &[Issue]) -> u8 {
        // Coupling issues are informational, minor score impact
        let coupling_issues = issues.len();
        let deduction = (coupling_issues as i32 * 2).min(10);
        (25 - deduction).max(0) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::TypeScriptParser;

    #[test]
    fn test_extract_imports() {
        let source = r#"
            import { add, subtract } from './math';
            import { UserService } from '../services/user';
            import { describe, it, expect } from 'vitest';
            
            describe('math', () => {
                it('adds', () => {
                    expect(add(1, 2)).toBe(3);
                });
            });
        "#;

        let mut parser = TypeScriptParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let imports = CouplingAnalysisRule::extract_test_imports(source, &tree);

        assert!(
            imports.len() >= 2,
            "Expected at least 2 imports, got {}",
            imports.len()
        );

        // Check that we found the math import
        let math_import = imports.iter().find(|i| i.source.contains("math"));
        assert!(math_import.is_some(), "Should find math import");

        if let Some(mi) = math_import {
            assert!(
                mi.names.contains(&"add".to_string()),
                "Should have 'add' in imports"
            );
        }
    }

    #[test]
    fn test_find_unused_imports() {
        let source = r#"
            import { add, subtract, multiply } from './math';
            
            describe('math', () => {
                it('adds numbers', () => {
                    expect(add(1, 2)).toBe(3);
                });
            });
        "#;

        let mut parser = TypeScriptParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let imports = CouplingAnalysisRule::extract_test_imports(source, &tree);
        let used = CouplingAnalysisRule::find_used_imports(source, &imports);

        assert!(used.contains("add"), "'add' should be detected as used");
        // subtract and multiply are only in import, so only 1 occurrence
        assert!(
            !used.contains("subtract"),
            "'subtract' should be detected as unused"
        );
        assert!(
            !used.contains("multiply"),
            "'multiply' should be detected as unused"
        );
    }
}
