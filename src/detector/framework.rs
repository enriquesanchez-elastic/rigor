//! Test framework detection

use crate::{TestFramework, TestType};
use std::path::Path;
use tree_sitter::{Node, Tree};

/// Detects test frameworks from import statements and patterns
pub struct FrameworkDetector<'a> {
    source: &'a str,
}

impl<'a> FrameworkDetector<'a> {
    /// Create a new framework detector
    pub fn new(source: &'a str) -> Self {
        Self { source }
    }

    /// Detect the test framework used in the file
    pub fn detect(&self, tree: &Tree) -> TestFramework {
        // First, check imports
        if let Some(framework) = self.detect_from_imports(tree.root_node()) {
            return framework;
        }

        // Fall back to pattern detection
        self.detect_from_patterns(tree.root_node())
    }

    fn detect_from_imports(&self, node: Node) -> Option<TestFramework> {
        if node.kind() == "import_statement" {
            let source_text = self.get_import_source(node)?;

            // Check for framework-specific imports
            if source_text.contains("vitest") {
                return Some(TestFramework::Vitest);
            }
            if source_text.contains("@playwright") || source_text.contains("playwright") {
                return Some(TestFramework::Playwright);
            }
            if source_text.contains("cypress") {
                return Some(TestFramework::Cypress);
            }
            if source_text.contains("@jest") || source_text == "jest" {
                return Some(TestFramework::Jest);
            }
            if source_text.contains("mocha") || source_text.contains("chai") {
                return Some(TestFramework::Mocha);
            }
        }

        // Recurse into children
        for child in node.named_children(&mut node.walk()) {
            if let Some(framework) = self.detect_from_imports(child) {
                return Some(framework);
            }
        }

        None
    }

    fn get_import_source(&self, import_node: Node) -> Option<String> {
        for child in import_node.named_children(&mut import_node.walk()) {
            if child.kind() == "string" {
                let text = self.node_text(child);
                // Remove quotes
                let trimmed = text.trim_matches(|c| c == '"' || c == '\'');
                return Some(trimmed.to_string());
            }
        }
        None
    }

    fn detect_from_patterns(&self, _node: Node) -> TestFramework {
        // Look for framework-specific patterns in the code
        let source_lower = self.source.to_lowercase();

        // Playwright patterns
        if source_lower.contains("page.goto")
            || source_lower.contains("page.click")
            || source_lower.contains("browser.newpage")
        {
            return TestFramework::Playwright;
        }

        // Cypress patterns
        if source_lower.contains("cy.visit")
            || source_lower.contains("cy.get")
            || source_lower.contains("cy.contains")
            || source_lower.contains("cy.should")
        {
            return TestFramework::Cypress;
        }

        // Jest patterns (most common, check last)
        if source_lower.contains("jest.mock")
            || source_lower.contains("jest.fn")
            || source_lower.contains("jest.spyon")
        {
            return TestFramework::Jest;
        }

        // Vitest patterns
        if source_lower.contains("vi.mock") || source_lower.contains("vi.fn") {
            return TestFramework::Vitest;
        }

        // Mocha patterns
        if source_lower.contains("assert.equal")
            || source_lower.contains("assert.strictequal")
            || source_lower.contains("should.")
        {
            return TestFramework::Mocha;
        }

        // Default: If we see expect(), assume Jest (most common)
        if source_lower.contains("expect(") {
            return TestFramework::Jest;
        }

        TestFramework::Unknown
    }

    fn node_text(&self, node: Node) -> &str {
        node.utf8_text(self.source.as_bytes()).unwrap_or("")
    }
    
    /// Detect the test type based on file path, framework, and content
    pub fn detect_test_type(&self, file_path: &Path, framework: TestFramework) -> TestType {
        let path_str = file_path.to_string_lossy().to_lowercase();
        let source_lower = self.source.to_lowercase();
        
        // Check file path patterns first (most reliable)
        if path_str.contains("e2e") || path_str.contains(".e2e.") || path_str.contains("/e2e/") {
            return TestType::E2e;
        }
        if path_str.contains(".cy.") || path_str.contains("/cypress/") {
            return TestType::E2e;
        }
        if path_str.contains("integration") || path_str.contains(".integration.") {
            return TestType::Integration;
        }
        if path_str.contains("component") || path_str.contains(".component.") {
            return TestType::Component;
        }
        
        // Check framework (E2E frameworks)
        if matches!(framework, TestFramework::Cypress | TestFramework::Playwright) {
            return TestType::E2e;
        }
        
        // Check content patterns for component tests
        if source_lower.contains("@testing-library") 
            || source_lower.contains("render(")
            || source_lower.contains("screen.get")
            || source_lower.contains("fireEvent")
            || source_lower.contains("userevent")
        {
            return TestType::Component;
        }
        
        // Check for Cypress/E2E patterns in content
        if source_lower.contains("cy.visit")
            || source_lower.contains("cy.get")
            || source_lower.contains("page.goto")
            || source_lower.contains("page.click")
        {
            return TestType::E2e;
        }
        
        // Check for integration test patterns
        if source_lower.contains("supertest")
            || source_lower.contains("request(app)")
            || source_lower.contains("database")
            || source_lower.contains("mongodb")
            || source_lower.contains("postgres")
        {
            return TestType::Integration;
        }
        
        // Default to unit test
        TestType::Unit
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::TypeScriptParser;

    #[test]
    fn test_detect_vitest() {
        let source = r#"
            import { describe, it, expect } from 'vitest';

            describe('test', () => {
                it('works', () => {
                    expect(1).toBe(1);
                });
            });
        "#;

        let mut parser = TypeScriptParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let detector = FrameworkDetector::new(source);

        assert_eq!(detector.detect(&tree), TestFramework::Vitest);
    }

    #[test]
    fn test_detect_jest_pattern() {
        let source = r#"
            describe('test', () => {
                it('works', () => {
                    const mock = jest.fn();
                    expect(mock).toHaveBeenCalled();
                });
            });
        "#;

        let mut parser = TypeScriptParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let detector = FrameworkDetector::new(source);

        assert_eq!(detector.detect(&tree), TestFramework::Jest);
    }

    #[test]
    fn test_detect_playwright() {
        let source = r#"
            import { test, expect } from '@playwright/test';

            test('navigation', async ({ page }) => {
                await page.goto('/');
                expect(page).toHaveTitle('Home');
            });
        "#;

        let mut parser = TypeScriptParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let detector = FrameworkDetector::new(source);

        assert_eq!(detector.detect(&tree), TestFramework::Playwright);
    }

    #[test]
    fn test_detect_cypress() {
        let source = r#"
            describe('login', () => {
                it('navigates to home', () => {
                    cy.visit('/');
                    cy.get('[data-testid=submit]').should('be.visible');
                });
            });
        "#;

        let mut parser = TypeScriptParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let detector = FrameworkDetector::new(source);

        assert_eq!(detector.detect(&tree), TestFramework::Cypress);
    }

    #[test]
    fn test_detect_cypress_import() {
        let source = r#"
            import 'cypress';

            describe('suite', () => {
                it('works', () => {
                    cy.get('button').should('exist');
                });
            });
        "#;

        let mut parser = TypeScriptParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let detector = FrameworkDetector::new(source);

        assert_eq!(detector.detect(&tree), TestFramework::Cypress);
    }
}
