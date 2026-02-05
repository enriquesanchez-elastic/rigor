//! TypeScript parser using tree-sitter

use anyhow::{Context, Result};
use std::path::Path;
use tree_sitter::{Language, Parser, Tree};

/// Parser for TypeScript files using tree-sitter
pub struct TypeScriptParser {
    parser: Parser,
}

impl TypeScriptParser {
    /// Create a new TypeScript parser
    pub fn new() -> Result<Self> {
        let mut parser = Parser::new();
        let language: Language = tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into();
        parser
            .set_language(&language)
            .context("Failed to set TypeScript language")?;
        Ok(Self { parser })
    }

    /// Create a new TSX parser
    pub fn new_tsx() -> Result<Self> {
        let mut parser = Parser::new();
        let language: Language = tree_sitter_typescript::LANGUAGE_TSX.into();
        parser
            .set_language(&language)
            .context("Failed to set TSX language")?;
        Ok(Self { parser })
    }

    /// Create a parser based on file extension
    pub fn for_file(path: &Path) -> Result<Self> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        match ext {
            "tsx" => Self::new_tsx(),
            _ => Self::new(),
        }
    }

    /// Parse source code into a syntax tree
    pub fn parse(&mut self, source: &str) -> Result<Tree> {
        self.parser
            .parse(source, None)
            .context("Failed to parse TypeScript source")
    }

    /// Get the tree-sitter language for TypeScript
    pub fn language() -> Language {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
    }

    /// Get the tree-sitter language for TSX
    pub fn language_tsx() -> Language {
        tree_sitter_typescript::LANGUAGE_TSX.into()
    }
}

impl Default for TypeScriptParser {
    fn default() -> Self {
        Self::new().expect("Failed to create TypeScript parser")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let mut parser = TypeScriptParser::new().unwrap();
        let tree = parser.parse("const x = 1;").unwrap();
        assert!(!tree.root_node().has_error());
    }

    #[test]
    fn test_parse_function() {
        let mut parser = TypeScriptParser::new().unwrap();
        let source = r#"
            function greet(name: string): string {
                return `Hello, ${name}!`;
            }
        "#;
        let tree = parser.parse(source).unwrap();
        assert!(!tree.root_node().has_error());
    }
}
