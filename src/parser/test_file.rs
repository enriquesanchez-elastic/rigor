//! Test file parser - extracts test cases and assertions

use crate::{Assertion, AssertionKind, Location, TestCase, TestStats};
use tree_sitter::{Node, Tree};

/// Parser for extracting test structure from TypeScript test files
pub struct TestFileParser<'a> {
    source: &'a str,
}

impl<'a> TestFileParser<'a> {
    /// Create a new test file parser
    pub fn new(source: &'a str) -> Self {
        Self { source }
    }

    /// Extract all test cases from a parsed tree
    pub fn extract_tests(&self, tree: &Tree) -> Vec<TestCase> {
        let mut tests = Vec::new();
        self.visit_node(tree.root_node(), &mut tests, None);
        tests
    }

    /// Extract test statistics
    pub fn extract_stats(&self, tree: &Tree) -> TestStats {
        let tests = self.extract_tests(tree);

        TestStats {
            total_tests: tests.len(),
            skipped_tests: tests.iter().filter(|t| t.is_skipped).count(),
            async_tests: tests.iter().filter(|t| t.is_async).count(),
            total_assertions: tests.iter().map(|t| t.assertions.len()).sum(),
            describe_blocks: self.count_describe_blocks(tree.root_node()),
            ..Default::default()
        }
    }

    fn visit_node(&self, node: Node, tests: &mut Vec<TestCase>, current_describe: Option<&str>) {
        // Check if this is a test or describe call
        if node.kind() == "call_expression" {
            if let Some(test) = self.try_parse_test(node, current_describe) {
                tests.push(test);
                return; // Don't recurse into test body for nested tests
            }

            if let Some(describe_name) = self.try_parse_describe(node) {
                // Recurse into describe block with new context
                if let Some(args) = node.child_by_field_name("arguments") {
                    let mut cursor = args.walk();
                    for child in args.named_children(&mut cursor) {
                        self.visit_node(child, tests, Some(&describe_name));
                    }
                }
                return;
            }
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.visit_node(child, tests, current_describe);
        }
    }

    fn try_parse_test(&self, node: Node, describe_block: Option<&str>) -> Option<TestCase> {
        let function = node.child_by_field_name("function")?;
        let fn_name = self.node_text(function);

        // Check for test/it and variants
        let (is_skipped, is_test) = match fn_name {
            "it" | "test" => (false, true),
            "it.skip" | "test.skip" | "xit" | "xtest" => (true, true),
            "it.only" | "test.only" | "fit" | "ftest" => (false, true),
            _ => {
                // Check for member expression like it.skip
                if function.kind() == "member_expression" {
                    let obj = self.node_text(function.child_by_field_name("object")?);
                    let prop = self.node_text(function.child_by_field_name("property")?);
                    match (obj, prop) {
                        ("it" | "test", "skip") => (true, true),
                        ("it" | "test", "only") => (false, true),
                        ("it" | "test", "todo") => (true, true),
                        _ => (false, false),
                    }
                } else {
                    (false, false)
                }
            }
        };

        if !is_test {
            return None;
        }

        let args = node.child_by_field_name("arguments")?;
        let mut cursor = args.walk();
        let mut args_iter = args.named_children(&mut cursor);

        // First argument is the test name
        let name_node = args_iter.next()?;
        let name = self.extract_string_value(name_node);

        // Second argument is the test function
        let body_node = args_iter.next()?;
        let is_async = self.is_async_function(body_node);
        let assertions = self.extract_assertions(body_node);

        let location = Location::new(
            node.start_position().row + 1,
            node.start_position().column + 1,
        )
        .with_end(node.end_position().row + 1, node.end_position().column + 1);

        Some(TestCase {
            name,
            location,
            is_async,
            is_skipped,
            assertions,
            describe_block: describe_block.map(String::from),
        })
    }

    fn try_parse_describe(&self, node: Node) -> Option<String> {
        let function = node.child_by_field_name("function")?;
        let fn_name = self.node_text(function);

        // Check for describe and variants
        let is_describe = matches!(fn_name, "describe" | "describe.skip" | "describe.only");

        if !is_describe {
            // Check for member expression
            if function.kind() == "member_expression" {
                let obj = self.node_text(function.child_by_field_name("object")?);
                if obj != "describe" {
                    return None;
                }
            } else {
                return None;
            }
        }

        let args = node.child_by_field_name("arguments")?;
        let mut cursor = args.walk();
        let name_node = args.named_children(&mut cursor).next()?;
        Some(self.extract_string_value(name_node))
    }

    fn extract_assertions(&self, node: Node) -> Vec<Assertion> {
        let mut assertions = Vec::new();
        self.visit_for_assertions(node, &mut assertions);
        assertions
    }

    fn visit_for_assertions(&self, node: Node, assertions: &mut Vec<Assertion>) {
        if node.kind() == "call_expression" {
            if let Some(assertion) = self.try_parse_assertion(node) {
                assertions.push(assertion);
            }
        }

        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            self.visit_for_assertions(child, assertions);
        }
    }

    fn try_parse_assertion(&self, node: Node) -> Option<Assertion> {
        let function = node.child_by_field_name("function")?;

        // Look for expect(...).matcher() pattern
        if function.kind() == "member_expression" {
            let property = function.child_by_field_name("property")?;
            let method_name = self.node_text(property);
            let object = function.child_by_field_name("object")?;

            // Check if it's a negated assertion (expect().not.matcher())
            let (kind, is_negated) = if object.kind() == "member_expression" {
                let inner_prop = object.child_by_field_name("property")?;
                if self.node_text(inner_prop) == "not" {
                    (self.method_to_assertion_kind(method_name), true)
                } else {
                    (self.method_to_assertion_kind(method_name), false)
                }
            } else {
                (self.method_to_assertion_kind(method_name), false)
            };

            // Verify this is an expect() call
            let expect_call = if is_negated {
                object.child_by_field_name("object")?
            } else {
                object
            };

            if expect_call.kind() == "call_expression" {
                let expect_fn = expect_call.child_by_field_name("function")?;
                if self.node_text(expect_fn) == "expect" {
                    let final_kind = if is_negated {
                        AssertionKind::Negated(Box::new(kind))
                    } else {
                        kind
                    };

                    let location = Location::new(
                        node.start_position().row + 1,
                        node.start_position().column + 1,
                    );

                    return Some(Assertion {
                        kind: final_kind.clone(),
                        quality: final_kind.quality(),
                        location,
                        raw: self.node_text(node).to_string(),
                    });
                }
            }
        }

        // Check for assert.* pattern
        if function.kind() == "member_expression" {
            let object = function.child_by_field_name("object")?;
            if self.node_text(object) == "assert" {
                let location = Location::new(
                    node.start_position().row + 1,
                    node.start_position().column + 1,
                );

                return Some(Assertion {
                    kind: AssertionKind::Assert,
                    quality: AssertionKind::Assert.quality(),
                    location,
                    raw: self.node_text(node).to_string(),
                });
            }
        }

        // Check for Cypress .should() pattern: cy.get(...).should('exist'), .should('be.visible'), etc.
        if function.kind() == "member_expression" {
            let property = function.child_by_field_name("property")?;
            let prop_name = self.node_text(property);

            if prop_name == "should" {
                let args = node.child_by_field_name("arguments")?;
                let mut cursor = args.walk();
                let first_arg = args.named_children(&mut cursor).next()?;
                let assertion_type = self.extract_string_value(first_arg);
                let kind = self.cypress_should_to_assertion_kind(&assertion_type);

                let location = Location::new(
                    node.start_position().row + 1,
                    node.start_position().column + 1,
                );

                return Some(Assertion {
                    kind: kind.clone(),
                    quality: kind.quality(),
                    location,
                    raw: self.node_text(node).to_string(),
                });
            }
        }

        // Check for Cypress implicit assertions (cy.contains, cy.url, cy.intercept, etc.)
        if let Some(assertion) = self.try_parse_cypress_implicit_assertion(node) {
            return Some(assertion);
        }

        None
    }

    /// Try to parse Cypress implicit assertions (commands that implicitly assert)
    fn try_parse_cypress_implicit_assertion(&self, node: Node) -> Option<Assertion> {
        let function = node.child_by_field_name("function")?;

        // Check if this is a Cypress command chain rooted at 'cy'
        if !self.is_cypress_chain(function) {
            return None;
        }

        // Get the final method name in the chain
        let method_name = if function.kind() == "member_expression" {
            let property = function.child_by_field_name("property")?;
            self.node_text(property)
        } else {
            return None;
        };

        let kind = match method_name {
            "contains" => AssertionKind::CyContains,
            "url" => AssertionKind::CyUrl,
            "intercept" => AssertionKind::CyIntercept,
            "visit" => AssertionKind::CyVisit,
            "click" | "type" | "clear" | "check" | "uncheck" | "select" | "focus" | "blur"
            | "submit" | "trigger" | "scrollTo" | "scrollIntoView" | "dblclick" | "rightclick" => {
                AssertionKind::CyAction
            }
            "get" | "find" | "first" | "last" | "eq" | "parent" | "children" | "siblings" => {
                // cy.get() without .should() is a weak implicit assertion
                // Check if the parent call has .should() - if so, don't count twice
                if self.has_should_in_chain(node) {
                    return None;
                }
                AssertionKind::CyGetImplicit
            }
            _ => return None,
        };

        let location = Location::new(
            node.start_position().row + 1,
            node.start_position().column + 1,
        );

        Some(Assertion {
            kind: kind.clone(),
            quality: kind.quality(),
            location,
            raw: self.node_text(node).to_string(),
        })
    }

    /// Check if a call expression is part of a Cypress chain (rooted at 'cy')
    fn is_cypress_chain(&self, node: Node) -> bool {
        match node.kind() {
            "identifier" => self.node_text(node) == "cy",
            "member_expression" => {
                if let Some(object) = node.child_by_field_name("object") {
                    self.is_cypress_chain(object)
                } else {
                    false
                }
            }
            "call_expression" => {
                if let Some(function) = node.child_by_field_name("function") {
                    self.is_cypress_chain(function)
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Check if any parent/sibling in the chain has a .should() call
    fn has_should_in_chain(&self, node: Node) -> bool {
        // Walk up to find if we're part of a longer chain that includes .should()
        if let Some(parent) = node.parent() {
            if parent.kind() == "member_expression" {
                if let Some(property) = parent.child_by_field_name("property") {
                    if self.node_text(property) == "should" {
                        return true;
                    }
                }
                // Check further up the chain
                if let Some(grandparent) = parent.parent() {
                    if grandparent.kind() == "call_expression" {
                        return self.has_should_in_chain(grandparent);
                    }
                }
            }
        }
        false
    }

    fn cypress_should_to_assertion_kind(&self, assertion_type: &str) -> AssertionKind {
        match assertion_type {
            "exist" => AssertionKind::CyShouldExist,
            "be.visible" => AssertionKind::CyShouldBeVisible,
            "have.text" => AssertionKind::CyShouldHaveText,
            "contain" | "contain.text" => AssertionKind::CyShouldContain,
            "have.length" => AssertionKind::CyShouldHaveLength,
            "eq" | "equal" => AssertionKind::CyShouldEqual,
            "be.disabled" => AssertionKind::CyShouldBeDisabled,
            "have.attr" | "have.attribute" => AssertionKind::CyShouldHaveAttr,
            // URL-related assertions (cy.url().should())
            "include" => AssertionKind::CyUrl,
            "match" => AssertionKind::CyUrl,
            // Value assertions
            "have.value" => AssertionKind::CyShouldEqual,
            "have.class" => AssertionKind::CyShouldHaveAttr,
            "have.css" => AssertionKind::CyShouldHaveAttr,
            // State assertions
            "be.enabled" | "be.checked" | "be.selected" | "be.focused" => {
                AssertionKind::CyShouldBeVisible
            }
            "not.exist" | "not.be.visible" | "be.empty" | "be.hidden" => {
                AssertionKind::CyShouldExist
            }
            other => AssertionKind::Unknown(other.to_string()),
        }
    }

    fn method_to_assertion_kind(&self, method: &str) -> AssertionKind {
        match method {
            "toBe" => AssertionKind::ToBe,
            "toEqual" => AssertionKind::ToEqual,
            "toStrictEqual" => AssertionKind::ToStrictEqual,
            "toBeDefined" => AssertionKind::ToBeDefined,
            "toBeUndefined" => AssertionKind::ToBeUndefined,
            "toBeNull" => AssertionKind::ToBeNull,
            "toBeTruthy" => AssertionKind::ToBeTruthy,
            "toBeFalsy" => AssertionKind::ToBeFalsy,
            "toThrow" => AssertionKind::ToThrow,
            "toThrowError" => AssertionKind::ToThrow,
            "toHaveBeenCalled" => AssertionKind::ToHaveBeenCalled,
            "toHaveBeenCalledWith" => AssertionKind::ToHaveBeenCalled,
            "toContain" => AssertionKind::ToContain,
            "toMatch" => AssertionKind::ToMatch,
            "toHaveLength" => AssertionKind::ToHaveLength,
            "toBeGreaterThan" => AssertionKind::ToBeGreaterThan,
            "toBeGreaterThanOrEqual" => AssertionKind::ToBeGreaterThan,
            "toBeLessThan" => AssertionKind::ToBeLessThan,
            "toBeLessThanOrEqual" => AssertionKind::ToBeLessThan,
            "toHaveProperty" => AssertionKind::ToHaveProperty,
            "toMatchSnapshot" => AssertionKind::ToMatchSnapshot,
            "toMatchInlineSnapshot" => AssertionKind::ToMatchInlineSnapshot,
            "toHaveBeenCalledTimes" => AssertionKind::ToHaveBeenCalledTimes,
            "toHaveBeenNthCalledWith" => AssertionKind::ToHaveBeenNthCalledWith,
            "toBeInstanceOf" => AssertionKind::ToBeInstanceOf,
            "toHaveClass" => AssertionKind::ToHaveClass,
            "toBeVisible" => AssertionKind::ToBeVisible,
            "toHaveText" => AssertionKind::ToHaveText,
            other => AssertionKind::Unknown(other.to_string()),
        }
    }

    fn is_async_function(&self, node: Node) -> bool {
        match node.kind() {
            "arrow_function" | "function_expression" => {
                // Check for async keyword
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "async" {
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }

    fn extract_string_value(&self, node: Node) -> String {
        let text = self.node_text(node);
        // Remove quotes
        if (text.starts_with('"') && text.ends_with('"'))
            || (text.starts_with('\'') && text.ends_with('\''))
            || (text.starts_with('`') && text.ends_with('`'))
        {
            text[1..text.len() - 1].to_string()
        } else {
            text.to_string()
        }
    }

    fn node_text(&self, node: Node) -> &str {
        node.utf8_text(self.source.as_bytes()).unwrap_or("")
    }

    fn count_describe_blocks(&self, node: Node) -> usize {
        let mut count = 0;

        if node.kind() == "call_expression" {
            if let Some(function) = node.child_by_field_name("function") {
                let fn_name = self.node_text(function);
                if fn_name == "describe" || fn_name.starts_with("describe.") {
                    count += 1;
                }
            }
        }

        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            count += self.count_describe_blocks(child);
        }

        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::TypeScriptParser;
    use crate::AssertionKind;

    #[test]
    fn test_extract_simple_test() {
        let source = r#"
            it('should work', () => {
                expect(1).toBe(1);
            });
        "#;

        let mut parser = TypeScriptParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let test_parser = TestFileParser::new(source);
        let tests = test_parser.extract_tests(&tree);

        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].name, "should work");
        assert!(!tests[0].is_async);
        assert!(!tests[0].is_skipped);
        assert_eq!(tests[0].assertions.len(), 1);
    }

    #[test]
    fn test_extract_skipped_test() {
        let source = r#"
            it.skip('should be skipped', () => {
                expect(1).toBe(1);
            });
        "#;

        let mut parser = TypeScriptParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let test_parser = TestFileParser::new(source);
        let tests = test_parser.extract_tests(&tree);

        assert_eq!(tests.len(), 1);
        assert!(tests[0].is_skipped);
    }

    #[test]
    fn test_extract_async_test() {
        let source = r#"
            it('async test', async () => {
                const result = await fetchData();
                expect(result).toBeDefined();
            });
        "#;

        let mut parser = TypeScriptParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let test_parser = TestFileParser::new(source);
        let tests = test_parser.extract_tests(&tree);

        assert_eq!(tests.len(), 1);
        assert!(tests[0].is_async);
    }

    #[test]
    fn test_extract_cypress_assertions() {
        let source = r#"
            it('cypress assertions', () => {
                cy.get('.btn').should('exist');
                cy.get('.btn').should('be.visible');
                cy.get('h1').should('have.text', 'Welcome');
            });
        "#;

        let mut parser = TypeScriptParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let test_parser = TestFileParser::new(source);
        let tests = test_parser.extract_tests(&tree);

        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].assertions.len(), 3);
        assert!(matches!(
            tests[0].assertions[0].kind,
            AssertionKind::CyShouldExist
        ));
        assert!(matches!(
            tests[0].assertions[1].kind,
            AssertionKind::CyShouldBeVisible
        ));
        assert!(matches!(
            tests[0].assertions[2].kind,
            AssertionKind::CyShouldHaveText
        ));
    }

    #[test]
    fn test_extract_cypress_implicit_assertions() {
        let source = r#"
            it('cypress implicit assertions', () => {
                cy.visit('/dashboard');
                cy.contains('Welcome');
                cy.url().should('include', '/dashboard');
                cy.intercept('GET', '/api/user').as('getUser');
                cy.get('.button').click();
            });
        "#;

        let mut parser = TypeScriptParser::new().unwrap();
        let tree = parser.parse(source).unwrap();
        let test_parser = TestFileParser::new(source);
        let tests = test_parser.extract_tests(&tree);

        assert_eq!(tests.len(), 1);
        // Should detect: visit, contains, url().should(), intercept, get().click()
        assert!(
            tests[0].assertions.len() >= 4,
            "Expected at least 4 assertions, got {}",
            tests[0].assertions.len()
        );

        // Check for CyVisit
        assert!(
            tests[0]
                .assertions
                .iter()
                .any(|a| matches!(a.kind, AssertionKind::CyVisit)),
            "Should detect cy.visit()"
        );
        // Check for CyContains
        assert!(
            tests[0]
                .assertions
                .iter()
                .any(|a| matches!(a.kind, AssertionKind::CyContains)),
            "Should detect cy.contains()"
        );
        // Check for CyIntercept
        assert!(
            tests[0]
                .assertions
                .iter()
                .any(|a| matches!(a.kind, AssertionKind::CyIntercept)),
            "Should detect cy.intercept()"
        );
    }
}

#[cfg(test)]
mod proptest_tests {
    use super::*;
    use proptest::prelude::*;

    /// Strategy that generates random strings that look vaguely like TypeScript test content.
    fn arbitrary_ts_content() -> impl Strategy<Value = String> {
        prop::collection::vec(
            prop::sample::select(vec![
                "describe(",
                "it(",
                "test(",
                "expect(",
                "beforeEach(",
                "afterEach(",
                ")",
                "{",
                "}",
                ";",
                "\n",
                " ",
                ".",
                "'test'",
                "() => ",
                "true",
                "false",
                ".toBe(",
                ".toEqual(",
                ".toThrow(",
                ".toBeTruthy()",
                "const x = 1;",
            ]),
            0..30,
        )
        .prop_map(|parts| parts.join(""))
    }

    /// Build nested describe/it source for property tests.
    fn build_nested_source(depth: usize, test_count: usize) -> String {
        let mut source = String::new();
        for i in 0..depth {
            source.push_str("describe('level ");
            source.push_str(&i.to_string());
            source.push_str("', () => {\n");
        }
        for i in 0..test_count {
            source.push_str("  it('test ");
            source.push_str(&i.to_string());
            source.push_str("', () => { expect(true).toBe(true); });\n");
        }
        for _ in 0..depth {
            source.push_str("});\n");
        }
        source
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]

        #[test]
        fn parser_never_panics_on_arbitrary_input(ref input in ".{0,500}") {
            let mut ts_parser = crate::parser::TypeScriptParser::new().unwrap();
            if let Ok(tree) = ts_parser.parse(input) {
                let parser = TestFileParser::new(input);
                let _tests = parser.extract_tests(&tree);
                let _stats = parser.extract_stats(&tree);
            }
        }

        #[test]
        fn parser_never_panics_on_ts_like_input(ref input in arbitrary_ts_content()) {
            let mut ts_parser = crate::parser::TypeScriptParser::new().unwrap();
            if let Ok(tree) = ts_parser.parse(input) {
                let parser = TestFileParser::new(input);
                let _tests = parser.extract_tests(&tree);
                let _stats = parser.extract_stats(&tree);
            }
        }

        #[test]
        fn parser_never_panics_on_nested_describe(
            depth in 1usize..10,
            test_count in 1usize..5
        ) {
            let source = build_nested_source(depth, test_count);
            let mut ts_parser = crate::parser::TypeScriptParser::new().unwrap();
            if let Ok(tree) = ts_parser.parse(&source) {
                let parser = TestFileParser::new(&source);
                let tests = parser.extract_tests(&tree);
                prop_assert!(tests.len() <= test_count);
            }
        }
    }
}
