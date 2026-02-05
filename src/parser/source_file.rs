//! Source file parser - extracts functions, error conditions, and boundaries

use crate::{FunctionCoverage, Location};
use tree_sitter::{Node, Tree};

// --- New types for behavioral completeness, return path coverage, and side effects ---

/// Shape of a return value for completeness analysis
#[derive(Debug, Clone, Default)]
pub struct ValueShape {
    /// Object property names if return is an object literal
    pub object_keys: Vec<String>,
    /// True if return is or may be an array
    pub is_array: bool,
    /// True if return is a primitive (number, string, boolean, etc.)
    pub is_primitive: bool,
}

/// A parameter of a function
#[derive(Debug, Clone)]
pub struct Parameter {
    /// Parameter name
    pub name: String,
    /// Type annotation if present
    pub type_annotation: Option<String>,
}

/// A return statement inside a function
#[derive(Debug, Clone)]
pub struct ReturnStatement {
    /// Shape of the returned value if inferrable
    pub value_shape: Option<ValueShape>,
    /// Location in the file
    pub location: Location,
    /// True if this is an early return (e.g. inside if block)
    pub is_early_return: bool,
    /// Condition context for path analysis, e.g. "if x < 0"
    pub condition_context: Option<String>,
}

/// Kind of mutation (side effect)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MutationKind {
    ArrayPush,
    ArrayPop,
    ArraySplice,
    ArraySort,
    ArrayReverse,
    ArrayShift,
    ArrayUnshift,
    PropertyAssign,
    VariableAssign,
}

/// A mutation (side effect) detected in source
#[derive(Debug, Clone)]
pub struct Mutation {
    /// Variable or object being mutated (e.g. "cart.items", "state")
    pub target: String,
    /// Kind of mutation
    pub kind: MutationKind,
    /// Location in the file
    pub location: Location,
}

/// Detailed info about a function for meaningfulness analysis
#[derive(Debug, Clone)]
pub struct FunctionDetails {
    /// Function name
    pub name: String,
    /// Parameters
    pub parameters: Vec<Parameter>,
    /// Return type annotation if present
    pub return_type: Option<String>,
    /// All return statements in the function
    pub return_statements: Vec<ReturnStatement>,
    /// Mutations (side effects) in the function body
    pub mutations: Vec<Mutation>,
    /// Number of distinct return paths (branches leading to return)
    pub return_paths: usize,
    /// Cyclomatic complexity (branches + 1)
    pub cyclomatic_complexity: usize,
    /// Location in the file
    pub location: Location,
}

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

/// An exported item from a source file
#[derive(Debug, Clone)]
pub struct ExportedItem {
    /// Name of the exported item
    pub name: String,
    /// Kind of export (function, class, const, type)
    pub kind: ExportKind,
    /// Location in the file
    pub location: Location,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExportKind {
    Function,
    Class,
    Const,
    Type,
    Interface,
    Variable,
    Default,
    Other,
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

    /// Extract all exports from the source file
    pub fn extract_exports(&self, tree: &Tree) -> Vec<ExportedItem> {
        let mut exports = Vec::new();
        self.visit_for_exports(tree.root_node(), &mut exports);
        exports
    }

    /// Extract detailed function info for meaningfulness analysis
    pub fn extract_function_details(&self, tree: &Tree) -> Vec<FunctionDetails> {
        let mut functions = Vec::new();
        self.visit_for_function_details(tree.root_node(), &mut functions);
        functions
    }

    /// Calculate function coverage based on test file content
    pub fn calculate_coverage(&self, tree: &Tree, test_source: &str) -> FunctionCoverage {
        let exports = self.extract_exports(tree);
        let test_lower = test_source.to_lowercase();

        let mut tested = Vec::new();
        let mut untested = Vec::new();

        for export in &exports {
            let name_lower = export.name.to_lowercase();
            // Check if the export name appears in the test file
            // (simple heuristic - could be improved with import analysis)
            if test_lower.contains(&name_lower) {
                tested.push(export.name.clone());
            } else {
                untested.push(export.name.clone());
            }
        }

        let total = exports.len();
        let covered = tested.len();
        let percent = if total > 0 {
            ((covered * 100) / total) as u8
        } else {
            100 // No exports = 100% covered
        };

        FunctionCoverage {
            total_exports: total,
            covered_exports: covered,
            coverage_percent: percent,
            untested_exports: untested,
            tested_exports: tested,
        }
    }

    fn visit_for_exports(&self, node: Node, exports: &mut Vec<ExportedItem>) {
        match node.kind() {
            // export function name() {}
            // export const name = ...
            // export class Name {}
            "export_statement" => {
                self.extract_export_items(node, exports);
            }
            // export { name, name2 }
            "export_clause" => {
                for child in node.named_children(&mut node.walk()) {
                    if child.kind() == "export_specifier" {
                        if let Some(name) = child.child_by_field_name("name") {
                            let location = Location::new(
                                name.start_position().row + 1,
                                name.start_position().column + 1,
                            );
                            exports.push(ExportedItem {
                                name: self.node_text(name).to_string(),
                                kind: ExportKind::Other,
                                location,
                            });
                        }
                    }
                }
            }
            _ => {}
        }

        // Recurse
        for child in node.named_children(&mut node.walk()) {
            self.visit_for_exports(child, exports);
        }
    }

    fn extract_export_items(&self, export_node: Node, exports: &mut Vec<ExportedItem>) {
        for child in export_node.named_children(&mut export_node.walk()) {
            match child.kind() {
                "function_declaration" => {
                    if let Some(name) = child.child_by_field_name("name") {
                        let location = Location::new(
                            name.start_position().row + 1,
                            name.start_position().column + 1,
                        );
                        exports.push(ExportedItem {
                            name: self.node_text(name).to_string(),
                            kind: ExportKind::Function,
                            location,
                        });
                    }
                }
                "class_declaration" => {
                    if let Some(name) = child.child_by_field_name("name") {
                        let location = Location::new(
                            name.start_position().row + 1,
                            name.start_position().column + 1,
                        );
                        exports.push(ExportedItem {
                            name: self.node_text(name).to_string(),
                            kind: ExportKind::Class,
                            location,
                        });
                    }
                }
                "lexical_declaration" | "variable_declaration" => {
                    // export const/let/var name = ...
                    for decl in child.named_children(&mut child.walk()) {
                        if decl.kind() == "variable_declarator" {
                            if let Some(name) = decl.child_by_field_name("name") {
                                let location = Location::new(
                                    name.start_position().row + 1,
                                    name.start_position().column + 1,
                                );
                                exports.push(ExportedItem {
                                    name: self.node_text(name).to_string(),
                                    kind: ExportKind::Const,
                                    location,
                                });
                            }
                        }
                    }
                }
                "type_alias_declaration" => {
                    if let Some(name) = child.child_by_field_name("name") {
                        let location = Location::new(
                            name.start_position().row + 1,
                            name.start_position().column + 1,
                        );
                        exports.push(ExportedItem {
                            name: self.node_text(name).to_string(),
                            kind: ExportKind::Type,
                            location,
                        });
                    }
                }
                "interface_declaration" => {
                    if let Some(name) = child.child_by_field_name("name") {
                        let location = Location::new(
                            name.start_position().row + 1,
                            name.start_position().column + 1,
                        );
                        exports.push(ExportedItem {
                            name: self.node_text(name).to_string(),
                            kind: ExportKind::Interface,
                            location,
                        });
                    }
                }
                // export default ...
                _ if self.node_text(child) == "default" => {
                    // Check next sibling for the actual export
                    if let Some(sibling) = child.next_named_sibling() {
                        let name = match sibling.kind() {
                            "function_declaration" | "class_declaration" => sibling
                                .child_by_field_name("name")
                                .map(|n| self.node_text(n).to_string())
                                .unwrap_or_else(|| "default".to_string()),
                            _ => "default".to_string(),
                        };
                        let location = Location::new(
                            sibling.start_position().row + 1,
                            sibling.start_position().column + 1,
                        );
                        exports.push(ExportedItem {
                            name,
                            kind: ExportKind::Default,
                            location,
                        });
                    }
                }
                _ => {}
            }
        }
    }

    fn visit_for_throwables(
        &self,
        node: Node,
        functions: &mut Vec<ThrowableFunction>,
        current_fn: Option<&str>,
    ) {
        // Track current function context
        let fn_name = match node.kind() {
            "function_declaration" => node.child_by_field_name("name").map(|n| self.node_text(n)),
            "method_definition" => node.child_by_field_name("name").map(|n| self.node_text(n)),
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
            "function_declaration" | "method_definition" => node
                .child_by_field_name("name")
                .map(|n| self.node_text(n).to_string()),
            _ => context.map(String::from),
        };

        // Check for binary expressions with comparison operators
        if node.kind() == "binary_expression" {
            if let Some(op_node) = node.child_by_field_name("operator") {
                let operator = self.node_text(op_node);
                if matches!(
                    operator,
                    "<" | ">" | "<=" | ">=" | "==" | "===" | "!=" | "!=="
                ) {
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

    fn visit_for_function_details(&self, node: Node, functions: &mut Vec<FunctionDetails>) {
        let kind = node.kind();
        if matches!(
            kind,
            "function_declaration" | "method_definition" | "arrow_function" | "function_expression"
        ) {
            if let Some(details) = self.parse_function_details(node) {
                functions.push(details);
            }
            return; // Don't recurse into nested functions here; we'll get them in the full tree walk
        }

        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.visit_for_function_details(child, functions);
        }
    }

    fn parse_function_details(&self, node: Node) -> Option<FunctionDetails> {
        let name = self
            .get_function_name(node)
            .unwrap_or_else(|| "anonymous".to_string());
        let location = Location::new(
            node.start_position().row + 1,
            node.start_position().column + 1,
        );
        let parameters = self.extract_parameters(node);
        let return_type = self.extract_return_type(node);
        let body = node.child_by_field_name("body")?;
        let return_statements = self.extract_return_statements_from_body(body, None);
        let mutations = self.extract_mutations_from_node(body);
        let return_paths = self.count_return_paths(body);
        let cyclomatic_complexity = self.calculate_cyclomatic_complexity(body);

        Some(FunctionDetails {
            name,
            parameters,
            return_type,
            return_statements,
            mutations,
            return_paths: return_paths.max(1),
            cyclomatic_complexity: cyclomatic_complexity.max(1),
            location,
        })
    }

    fn get_function_name(&self, node: Node) -> Option<String> {
        match node.kind() {
            "function_declaration" | "method_definition" => node
                .child_by_field_name("name")
                .map(|n| self.node_text(n).to_string()),
            "arrow_function" | "function_expression" => node.parent().and_then(|parent| {
                if parent.kind() == "variable_declarator" {
                    parent
                        .child_by_field_name("name")
                        .map(|n| self.node_text(n).to_string())
                } else if parent.kind() == "method_definition" {
                    parent
                        .child_by_field_name("name")
                        .map(|n| self.node_text(n).to_string())
                } else {
                    Some("anonymous".to_string())
                }
            }),
            _ => None,
        }
    }

    fn extract_parameters(&self, node: Node) -> Vec<Parameter> {
        let mut params = Vec::new();
        let params_node = match node.child_by_field_name("parameters") {
            Some(p) => p,
            None => return params,
        };
        let mut cursor = params_node.walk();
        for child in params_node.named_children(&mut cursor) {
            if child.kind() == "identifier" {
                let name = self.node_text(child).to_string();
                if name != "this" {
                    let type_annotation = child
                        .parent()
                        .and_then(|p| p.child_by_field_name("type"))
                        .map(|t| self.node_text(t).to_string());
                    params.push(Parameter {
                        name,
                        type_annotation,
                    });
                }
            } else if child.kind() == "required_parameter" || child.kind() == "optional_parameter" {
                let name_node = child
                    .child_by_field_name("name")
                    .or_else(|| child.named_children(&mut child.walk()).next());
                if let Some(n) = name_node {
                    let name = self.node_text(n).to_string();
                    if !name.is_empty() && name != "this" {
                        let type_annotation = child
                            .child_by_field_name("type")
                            .map(|t| self.node_text(t).to_string());
                        params.push(Parameter {
                            name,
                            type_annotation,
                        });
                    }
                }
            }
        }
        params
    }

    fn extract_return_type(&self, node: Node) -> Option<String> {
        node.child_by_field_name("return_type")
            .map(|n| self.node_text(n).to_string())
    }

    fn extract_return_statements_from_body(
        &self,
        body: Node,
        condition_context: Option<&str>,
    ) -> Vec<ReturnStatement> {
        let mut statements = Vec::new();
        self.visit_for_returns(body, &mut statements, condition_context);
        statements
    }

    fn visit_for_returns(
        &self,
        node: Node,
        out: &mut Vec<ReturnStatement>,
        condition_context: Option<&str>,
    ) {
        if node.kind() == "return_statement" {
            let location = Location::new(
                node.start_position().row + 1,
                node.start_position().column + 1,
            );
            let value_node = node.child_by_field_name("value");
            let value_shape = value_node.and_then(|v| self.infer_value_shape(v));
            let is_early_return = condition_context.is_some();
            out.push(ReturnStatement {
                value_shape,
                location,
                is_early_return,
                condition_context: condition_context.map(String::from),
            });
            return;
        }

        if node.kind() == "if_statement" {
            let cond = node.child_by_field_name("condition");
            let consequence = node.child_by_field_name("consequence");
            let alternative = node.child_by_field_name("alternative");
            let ctx = cond.map(|c| self.node_text(c).to_string());
            if let Some(c) = consequence {
                self.visit_for_returns(c, out, ctx.as_deref());
            }
            if let Some(a) = alternative {
                self.visit_for_returns(a, out, ctx.as_deref());
            }
            return;
        }

        if node.kind() == "switch_statement" {
            let cases = node.child_by_field_name("body");
            if let Some(body) = cases {
                for child in body.named_children(&mut body.walk()) {
                    if child.kind() == "switch_case" || child.kind() == "default_case" {
                        for c in child.named_children(&mut child.walk()) {
                            self.visit_for_returns(c, out, None);
                        }
                    }
                }
            }
            return;
        }

        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.visit_for_returns(child, out, condition_context);
        }
    }

    fn infer_value_shape(&self, node: Node) -> Option<ValueShape> {
        match node.kind() {
            "object" | "object_type" => {
                let mut keys = Vec::new();
                for child in node.named_children(&mut node.walk()) {
                    if child.kind() == "pair" || child.kind() == "property_signature" {
                        if let Some(key) = child.child_by_field_name("key") {
                            let text = self.node_text(key);
                            if !text.is_empty() && text != "..." {
                                keys.push(text.to_string());
                            }
                        }
                    }
                }
                Some(ValueShape {
                    object_keys: keys,
                    is_array: false,
                    is_primitive: false,
                })
            }
            "array" | "array_type" => Some(ValueShape {
                object_keys: Vec::new(),
                is_array: true,
                is_primitive: false,
            }),
            "number" | "string" | "true" | "false" | "null" | "undefined" => Some(ValueShape {
                object_keys: Vec::new(),
                is_array: false,
                is_primitive: true,
            }),
            _ => None,
        }
    }

    fn extract_mutations_from_node(&self, node: Node) -> Vec<Mutation> {
        let mut mutations = Vec::new();
        self.visit_for_mutations(node, &mut mutations);
        mutations
    }

    fn visit_for_mutations(&self, node: Node, out: &mut Vec<Mutation>) {
        if node.kind() == "call_expression" {
            if let Some(function) = node.child_by_field_name("function") {
                let method = if function.kind() == "member_expression" {
                    let prop = function.child_by_field_name("property");
                    let obj = function.child_by_field_name("object");
                    let target = obj
                        .map(|o| self.node_text(o).to_string())
                        .unwrap_or_default();
                    let method_name = prop
                        .map(|p| self.node_text(p).to_string())
                        .unwrap_or_default();
                    let kind = match method_name.as_str() {
                        "push" => Some(MutationKind::ArrayPush),
                        "pop" => Some(MutationKind::ArrayPop),
                        "splice" => Some(MutationKind::ArraySplice),
                        "sort" => Some(MutationKind::ArraySort),
                        "reverse" => Some(MutationKind::ArrayReverse),
                        "shift" => Some(MutationKind::ArrayShift),
                        "unshift" => Some(MutationKind::ArrayUnshift),
                        _ => None,
                    };
                    kind.map(|k| (target, k))
                } else {
                    None
                };
                if let Some((target, kind)) = method {
                    let location = Location::new(
                        node.start_position().row + 1,
                        node.start_position().column + 1,
                    );
                    out.push(Mutation {
                        target,
                        kind,
                        location,
                    });
                }
            }
        }

        if node.kind() == "assignment_expression" {
            let left = node.child_by_field_name("left");
            let target = left
                .map(|l| self.node_text(l).to_string())
                .unwrap_or_default();
            let location = Location::new(
                node.start_position().row + 1,
                node.start_position().column + 1,
            );
            out.push(Mutation {
                target,
                kind: MutationKind::PropertyAssign,
                location,
            });
        }

        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.visit_for_mutations(child, out);
        }
    }

    fn count_return_paths(&self, body: Node) -> usize {
        let mut count = 0;
        self.visit_count_returns(body, &mut count);
        count
    }

    fn visit_count_returns(&self, node: Node, count: &mut usize) {
        if node.kind() == "return_statement" {
            *count += 1;
            return;
        }
        if node.kind() == "if_statement" {
            let consequence = node.child_by_field_name("consequence");
            let alternative = node.child_by_field_name("alternative");
            if let Some(c) = consequence {
                self.visit_count_returns(c, count);
            }
            if let Some(a) = alternative {
                self.visit_count_returns(a, count);
            }
            return;
        }
        if node.kind() == "switch_statement" {
            let body = node.child_by_field_name("body");
            if let Some(b) = body {
                for child in b.named_children(&mut b.walk()) {
                    if child.kind() == "switch_case" || child.kind() == "default_case" {
                        for c in child.named_children(&mut child.walk()) {
                            self.visit_count_returns(c, count);
                        }
                    }
                }
            }
            return;
        }
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.visit_count_returns(child, count);
        }
    }

    fn calculate_cyclomatic_complexity(&self, body: Node) -> usize {
        let mut complexity = 1;
        self.visit_complexity(body, &mut complexity);
        complexity
    }

    fn visit_complexity(&self, node: Node, complexity: &mut usize) {
        match node.kind() {
            "if_statement" | "else_clause" => *complexity += 1,
            "for_statement" | "for_in_statement" | "while_statement" | "do_statement" => {
                *complexity += 1
            }
            "switch_statement" => {
                let body = node.child_by_field_name("body");
                if let Some(b) = body {
                    let mut cursor = b.walk();
                    let cases: Vec<_> = b
                        .named_children(&mut cursor)
                        .filter(|c| c.kind() == "switch_case" || c.kind() == "default_case")
                        .collect();
                    for _ in cases {
                        *complexity += 1;
                    }
                }
            }
            "conditional_expression" => *complexity += 1,
            _ => {}
        }
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.visit_complexity(child, complexity);
        }
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

    #[test]
    fn test_extract_exports() {
        let source = r#"
            export function validateEmail(email: string): boolean {
                return email.includes('@');
            }
            
            export const MAX_LENGTH = 100;
            
            export class UserService {
                getUser(id: string) { return null; }
            }
            
            export interface User {
                id: string;
                name: string;
            }
            
            function privateHelper() { }
        "#;

        let mut parser = TypeScriptParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let source_parser = SourceFileParser::new(source);
        let exports = source_parser.extract_exports(&tree);

        assert!(
            exports.len() >= 4,
            "Expected at least 4 exports, got {}",
            exports.len()
        );

        let names: Vec<&str> = exports.iter().map(|e| e.name.as_str()).collect();
        assert!(
            names.contains(&"validateEmail"),
            "Should export validateEmail"
        );
        assert!(names.contains(&"MAX_LENGTH"), "Should export MAX_LENGTH");
        assert!(names.contains(&"UserService"), "Should export UserService");
        assert!(names.contains(&"User"), "Should export User interface");
        assert!(
            !names.contains(&"privateHelper"),
            "Should NOT export privateHelper"
        );
    }

    #[test]
    fn test_calculate_coverage() {
        let source = r#"
            export function add(a: number, b: number): number {
                return a + b;
            }
            
            export function subtract(a: number, b: number): number {
                return a - b;
            }
            
            export function multiply(a: number, b: number): number {
                return a * b;
            }
        "#;

        let test_source = r#"
            describe('math', () => {
                it('adds numbers', () => {
                    expect(add(1, 2)).toBe(3);
                });
                
                it('subtracts numbers', () => {
                    expect(subtract(5, 3)).toBe(2);
                });
            });
        "#;

        let mut parser = TypeScriptParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let source_parser = SourceFileParser::new(source);
        let coverage = source_parser.calculate_coverage(&tree, test_source);

        assert_eq!(coverage.total_exports, 3);
        assert_eq!(coverage.covered_exports, 2); // add and subtract
        assert!(coverage.untested_exports.contains(&"multiply".to_string()));
        assert!(coverage.coverage_percent == 66 || coverage.coverage_percent == 67);
    }

    #[test]
    fn test_extract_function_details() {
        let source = r#"
            function process(x: number): string {
                if (x < 0) return 'negative';
                if (x === 0) return 'zero';
                return 'positive';
            }
            
            export function addItem(cart: Cart, item: Item): number {
                cart.items.push(item);
                return cart.total;
            }
        "#;

        let mut parser = TypeScriptParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let source_parser = SourceFileParser::new(source);
        let details = source_parser.extract_function_details(&tree);

        assert!(!details.is_empty(), "Should extract at least one function");
        let process_fn = details.iter().find(|f| f.name == "process");
        assert!(process_fn.is_some(), "Should find process function");
        if let Some(f) = process_fn {
            assert!(f.return_paths >= 3, "process has 3 return paths");
            assert!(f.return_statements.len() >= 3);
        }
        let add_item = details.iter().find(|f| f.name == "addItem");
        if let Some(f) = add_item {
            assert!(!f.mutations.is_empty(), "addItem should have push mutation");
            assert!(f
                .mutations
                .iter()
                .any(|m| m.kind == MutationKind::ArrayPush));
        }
    }
}

#[cfg(test)]
mod proptest_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]

        #[test]
        fn source_parser_never_panics(ref input in ".{0,500}") {
            let mut ts_parser = crate::parser::TypeScriptParser::new().unwrap();
            if let Ok(tree) = ts_parser.parse(input) {
                let parser = SourceFileParser::new(input);
                let _exports = parser.extract_exports(&tree);
                let _throwables = parser.extract_throwable_functions(&tree);
                let _boundaries = parser.extract_boundary_conditions(&tree);
                let _details = parser.extract_function_details(&tree);
            }
        }
    }
}
