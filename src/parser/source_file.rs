//! Source file parser - extracts functions, error conditions, and boundaries

use crate::Location;
use tree_sitter::{Node, Tree};

/// A function that can throw an error
#[derive(Debug, Clone)]
pub struct ThrowableFunction {
    /// Name of the function
    pub name: String,
    /// Location in the file
    pub location: Location,
    /// Error types/messages thrown
    pub error_types: Vec<String>,
}

/// A boundary condition (numeric comparison)
#[derive(Debug, Clone)]
pub struct BoundaryCondition {
    /// The comparison operator
    pub operator: String,
    /// The boundary value (if constant)
    pub value: Option<String>,
    /// Location in the file
    pub location: Location,
    /// Context (function name or variable)
    pub context: String,
}

/// Parser for extracting analysis-relevant information from source files
pub struct SourceFileParser<'a> {
    source: &'a str,
}

impl<'a> SourceFileParser<'a> {
    /// Create a new source file parser
    pub fn new(source: &'a str) -> Self {
        Self { source }
    }

    /// Extract functions that can throw errors
    pub fn extract_throwable_functions(&self, tree: &Tree) -> Vec<ThrowableFunction> {
        let mut functions = Vec::new();
        self.visit_for_throwables(tree.root_node(), &mut functions, None);
        functions
    }

    /// Extract boundary conditions (numeric comparisons)
    pub fn extract_boundary_conditions(&self, tree: &Tree) -> Vec<BoundaryCondition> {
        let mut conditions = Vec::new();
        self.visit_for_boundaries(tree.root_node(), &mut conditions, None);
        conditions
    }

    fn visit_for_throwables(
        &self,
        node: Node,
        functions: &mut Vec<ThrowableFunction>,
        current_fn: Option<&str>,
    ) {
        // Track current function context
        let fn_name = match node.kind() {
            "function_declaration" => {
                node.child_by_field_name("name")
                    .map(|n| self.node_text(n))
            }
            "method_definition" => {
                node.child_by_field_name("name")
                    .map(|n| self.node_text(n))
            }
            "arrow_function" | "function_expression" => {
                // Try to get name from parent variable declaration
                if let Some(parent) = node.parent() {
                    if parent.kind() == "variable_declarator" {
                        parent
                            .child_by_field_name("name")
                            .map(|n| self.node_text(n))
                    } else {
                        current_fn
                    }
                } else {
                    current_fn
                }
            }
            _ => current_fn,
        };

        // Check for throw statements
        if node.kind() == "throw_statement" {
            if let Some(name) = fn_name {
                let error_type = self.extract_throw_type(node);

                // Check if we already have this function
                if let Some(existing) = functions.iter_mut().find(|f| f.name == name) {
                    if let Some(err) = error_type {
                        if !existing.error_types.contains(&err) {
                            existing.error_types.push(err);
                        }
                    }
                } else {
                    let location = Location::new(
                        node.start_position().row + 1,
                        node.start_position().column + 1,
                    );
                    functions.push(ThrowableFunction {
                        name: name.to_string(),
                        location,
                        error_types: error_type.into_iter().collect(),
                    });
                }
            }
        }

        // Recurse
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.visit_for_throwables(child, functions, fn_name);
        }
    }

    fn extract_throw_type(&self, throw_node: Node) -> Option<String> {
        // Get the expression being thrown
        let mut cursor = throw_node.walk();
        for child in throw_node.named_children(&mut cursor) {
            match child.kind() {
                "new_expression" => {
                    // new Error("message")
                    if let Some(constructor) = child.child_by_field_name("constructor") {
                        return Some(self.node_text(constructor).to_string());
                    }
                }
                "call_expression" => {
                    // Error("message") without new
                    if let Some(function) = child.child_by_field_name("function") {
                        return Some(self.node_text(function).to_string());
                    }
                }
                "identifier" => {
                    return Some(self.node_text(child).to_string());
                }
                _ => {}
            }
        }
        None
    }

    fn visit_for_boundaries(
        &self,
        node: Node,
        conditions: &mut Vec<BoundaryCondition>,
        context: Option<&str>,
    ) {
        // Track context (function/method name)
        let ctx = match node.kind() {
            "function_declaration" | "method_definition" => {
                node.child_by_field_name("name")
                    .map(|n| self.node_text(n).to_string())
            }
            _ => context.map(String::from),
        };

        // Check for binary expressions with comparison operators
        if node.kind() == "binary_expression" {
            if let Some(op_node) = node.child_by_field_name("operator") {
                let operator = self.node_text(op_node);
                if matches!(operator, "<" | ">" | "<=" | ">=" | "==" | "===" | "!=" | "!==") {
                    // Try to extract constant value
                    let value = self.extract_comparison_value(node);
                    let location = Location::new(
                        node.start_position().row + 1,
                        node.start_position().column + 1,
                    );

                    conditions.push(BoundaryCondition {
                        operator: operator.to_string(),
                        value,
                        location,
                        context: ctx.clone().unwrap_or_default(),
                    });
                }
            }
        }

        // Recurse
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.visit_for_boundaries(child, conditions, ctx.as_deref());
        }
    }

    fn extract_comparison_value(&self, binary_expr: Node) -> Option<String> {
        let left = binary_expr.child_by_field_name("left")?;
        let right = binary_expr.child_by_field_name("right")?;

        // Check if either side is a number literal
        for side in [left, right] {
            if side.kind() == "number" {
                return Some(self.node_text(side).to_string());
            }
        }

        None
    }

    fn node_text(&self, node: Node) -> &str {
        node.utf8_text(self.source.as_bytes()).unwrap_or("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::TypeScriptParser;

    #[test]
    fn test_extract_throw() {
        let source = r#"
            function validate(x: number) {
                if (x < 0) {
                    throw new Error("Value must be positive");
                }
            }
        "#;

        let mut parser = TypeScriptParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let source_parser = SourceFileParser::new(source);
        let throwables = source_parser.extract_throwable_functions(&tree);

        assert_eq!(throwables.len(), 1);
        assert_eq!(throwables[0].name, "validate");
        assert!(throwables[0].error_types.contains(&"Error".to_string()));
    }

    #[test]
    fn test_extract_boundary() {
        let source = r#"
            function checkAge(age: number) {
                if (age >= 18) {
                    return true;
                }
                return false;
            }
        "#;

        let mut parser = TypeScriptParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let source_parser = SourceFileParser::new(source);
        let boundaries = source_parser.extract_boundary_conditions(&tree);

        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].operator, ">=");
        assert_eq!(boundaries[0].value, Some("18".to_string()));
    }
}
