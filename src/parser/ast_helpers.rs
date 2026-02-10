//! Shared AST traversal helpers for analyzer rules.
//!
//! Provides utilities to walk the tree-sitter AST without re-implementing
//! traversal in each rule, and to avoid false positives from comments/strings.

use crate::Location;
use tree_sitter::{Node, Tree};

/// Info about a call expression: callee text and location.
#[derive(Debug, Clone)]
pub struct CallInfo {
    pub callee: String,
    pub location: Location,
    pub start_byte: usize,
    pub end_byte: usize,
}

/// Collect comment byte ranges (start, end) in source. Handles // and /* */.
fn comment_ranges(source: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let bytes = source.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() {
            if bytes[i] == b'/' && bytes[i + 1] == b'/' {
                let start = i;
                i += 2;
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                ranges.push((start, i));
                continue;
            }
            if bytes[i] == b'/' && bytes[i + 1] == b'*' {
                let start = i;
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                if i + 1 < bytes.len() {
                    i += 2;
                }
                ranges.push((start, i));
                continue;
            }
        }
        // Skip string literals so we don't treat "//" inside a string as comment
        if bytes[i] == b'"' || bytes[i] == b'\'' || bytes[i] == b'`' {
            let quote = bytes[i];
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\\' {
                    i += 2;
                    continue;
                }
                if bytes[i] == quote {
                    i += 1;
                    break;
                }
                if quote == b'`' && bytes[i] == b'$' && i + 1 < bytes.len() && bytes[i + 1] == b'{'
                {
                    i += 2;
                    let mut depth = 1u32;
                    while i < bytes.len() && depth > 0 {
                        if bytes[i] == b'{' {
                            depth += 1;
                        } else if bytes[i] == b'}' {
                            depth -= 1;
                        }
                        i += 1;
                    }
                    continue;
                }
                i += 1;
            }
            continue;
        }
        i += 1;
    }
    ranges
}

/// Returns true if the given node's byte range is entirely inside a comment.
pub fn is_inside_comment(node: Node, source: &str) -> bool {
    is_inside_comment_range(node.start_byte(), node.end_byte(), source)
}

/// Returns true if the byte range [start_byte, end_byte) is entirely inside a comment.
pub fn is_inside_comment_range(start_byte: usize, end_byte: usize, source: &str) -> bool {
    for (cstart, cend) in comment_ranges(source) {
        if start_byte >= cstart && end_byte <= cend {
            return true;
        }
    }
    false
}

/// Returns true if the given node is (or is inside) a string or template literal.
pub fn is_inside_string_literal(mut node: Node) -> bool {
    loop {
        let kind = node.kind();
        if kind == "string"
            || kind == "template_string"
            || kind == "template_literal"
            || kind.ends_with("string")
        {
            return true;
        }
        match node.parent() {
            Some(p) => node = p,
            None => return false,
        }
    }
}

/// Returns true if the byte range [start_byte, end_byte) is inside a string/template literal in the tree.
pub fn is_inside_string_literal_range(start_byte: usize, end_byte: usize, root: Node) -> bool {
    let kind = root.kind();
    if (kind == "string"
        || kind == "template_string"
        || kind == "template_literal"
        || kind.ends_with("string"))
        && root.start_byte() <= start_byte
        && end_byte <= root.end_byte()
    {
        return true;
    }
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        if is_inside_string_literal_range(start_byte, end_byte, child) {
            return true;
        }
    }
    false
}

/// Find the test body (it/test callback) that contains this node, if any.
/// Returns the arrow_function or function_expression node that is the test callback.
pub fn containing_test_body<'a>(
    node: Node<'a>,
    _tree: &'a Tree,
    source: &'a str,
) -> Option<Node<'a>> {
    let mut current = node;
    let start_byte = node.start_byte();
    let end_byte = node.end_byte();
    while let Some(parent) = current.parent() {
        if parent.kind() == "program" {
            break;
        }
        // Test callback is the second argument of it(...) or test(...)
        if parent.kind() == "call_expression" {
            if let Some(args) = parent.child_by_field_name("arguments") {
                let mut cursor = args.walk();
                let children: Vec<Node> = args.named_children(&mut cursor).collect();
                if children.len() >= 2 {
                    let callback = children[1];
                    let fn_node = parent.child_by_field_name("function")?;
                    let name = fn_node.utf8_text(source.as_bytes()).unwrap_or("");
                    let is_test = name == "it"
                        || name == "test"
                        || (fn_node.kind() == "member_expression"
                            && fn_node
                                .child_by_field_name("object")
                                .and_then(|o| o.utf8_text(source.as_bytes()).ok())
                                .map(|s| s == "it" || s == "test")
                                .unwrap_or(false));
                    let inside_callback =
                        callback.start_byte() <= start_byte && end_byte <= callback.end_byte();
                    if is_test && inside_callback {
                        return Some(callback);
                    }
                }
            }
        }
        current = parent;
    }
    None
}

/// Cyclomatic complexity: count branching nodes (if, switch, ternary, catch, &&, ||).
pub fn count_branches(node: Node) -> usize {
    let kind = node.kind();
    let one = if kind == "if_statement"
        || kind == "switch_statement"
        || kind == "ternary_expression"
        || kind == "catch_clause"
        || kind == "for_statement"
        || kind == "for_in_statement"
        || kind == "while_statement"
        || kind == "do_statement"
    {
        1
    } else if kind == "binary_expression" {
        // Without source we don't know if it's && or ||; use count_branches_with_source
        0
    } else {
        0
    };
    let mut sum = one;
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        sum += count_branches(child);
    }
    sum
}

/// Cyclomatic complexity using source to detect logical operators.
pub fn count_branches_with_source(node: Node, source: &str) -> usize {
    let kind = node.kind();
    let one = if kind == "if_statement"
        || kind == "switch_statement"
        || kind == "ternary_expression"
        || kind == "catch_clause"
        || kind == "for_statement"
        || kind == "for_in_statement"
        || kind == "while_statement"
        || kind == "do_statement"
    {
        1
    } else if kind == "binary_expression" {
        let op_node = node.child_by_field_name("operator");
        if let Some(op) = op_node {
            let op_text = op.utf8_text(source.as_bytes()).unwrap_or("");
            if op_text == "&&" || op_text == "||" {
                1
            } else {
                0
            }
        } else {
            0
        }
    } else {
        0
    };
    let mut sum = one;
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        sum += count_branches_with_source(child, source);
    }
    sum
}

/// Number of lines the node spans (1-indexed line count).
pub fn node_line_count(node: Node) -> usize {
    let start_row = node.start_position().row;
    let end_row = node.end_position().row;
    (end_row - start_row) + 1
}

/// Find all expect(...) call_expression nodes within the given body node.
pub fn find_assertions_in_body<'a>(body: Node<'a>, source: &'a [u8]) -> Vec<Node<'a>> {
    let mut out = Vec::new();
    visit_call_expressions(body, source, &mut out);
    out
}

fn visit_call_expressions<'a>(node: Node<'a>, source: &'a [u8], out: &mut Vec<Node<'a>>) {
    if node.kind() == "call_expression" {
        if let Some(func) = node.child_by_field_name("function") {
            let name = func.utf8_text(source).unwrap_or("");
            if name == "expect" {
                out.push(node);
                return;
            }
            if func.kind() == "member_expression" {
                if let Some(obj) = func.child_by_field_name("object") {
                    let obj_text = obj.utf8_text(source).unwrap_or("");
                    if obj_text == "expect" {
                        out.push(node);
                        return;
                    }
                }
            }
        }
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        visit_call_expressions(child, source, out);
    }
}

/// Find call expressions whose callee matches the given pattern.
/// Pattern is the full callee string, e.g. "jest.mock", "vi.mock", "Date.now".
pub fn find_call_expressions(tree: &Tree, source: &str, callee_pattern: &str) -> Vec<CallInfo> {
    let mut results = Vec::new();
    let root = tree.root_node();
    let bytes = source.as_bytes();
    visit_for_calls(root, bytes, callee_pattern, &mut results);
    results
}

fn visit_for_calls(node: Node, source: &[u8], callee_pattern: &str, out: &mut Vec<CallInfo>) {
    if node.kind() == "call_expression" {
        if let Some(func) = node.child_by_field_name("function") {
            let name = callee_text(func, source);
            if name == callee_pattern || name.starts_with(callee_pattern) {
                out.push(CallInfo {
                    callee: name,
                    location: Location::new(
                        node.start_position().row + 1,
                        node.start_position().column + 1,
                    ),
                    start_byte: node.start_byte(),
                    end_byte: node.end_byte(),
                });
            }
        }
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        visit_for_calls(child, source, callee_pattern, out);
    }
}

fn callee_text(node: Node, source: &[u8]) -> String {
    match node.kind() {
        "identifier" => node.utf8_text(source).unwrap_or("").to_string(),
        "member_expression" => {
            let obj = node
                .child_by_field_name("object")
                .map(|n| callee_text(n, source))
                .unwrap_or_default();
            let prop = node
                .child_by_field_name("property")
                .and_then(|n| n.utf8_text(source).ok())
                .unwrap_or_default();
            if obj.is_empty() {
                prop.to_string()
            } else {
                format!("{}.{}", obj, prop)
            }
        }
        _ => node.utf8_text(source).unwrap_or("").to_string(),
    }
}

/// Convert a tree-sitter Node to Location.
pub fn node_to_location(node: Node) -> Location {
    Location::new(
        node.start_position().row + 1,
        node.start_position().column + 1,
    )
    .with_end(node.end_position().row + 1, node.end_position().column + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comment_ranges_line() {
        let s = "foo(); // Date.now() here\nbar();";
        let ranges = comment_ranges(s);
        assert_eq!(ranges.len(), 1);
        assert!(ranges[0].0 < s.len() && ranges[0].1 <= s.len());
    }

    #[test]
    fn comment_ranges_block() {
        let s = "/* Date.now() */ x();";
        let ranges = comment_ranges(s);
        assert_eq!(ranges.len(), 1);
    }
}
