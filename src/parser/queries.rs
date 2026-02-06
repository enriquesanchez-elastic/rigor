//! Shared tree-sitter query library for analyzer rules.
//!
//! Compiles S-expression queries once and reuses them (query compilation cache).
//! Rules can use these queries for AST-based detection instead of regex/line scanning.
//!
//! **Migrated rules:** debug_code (console.*, debugger, it.only/test.only/describe.only).
//!
//! **To migrate another rule:** Add a `QueryId` variant and a query string in
//! `QueryCache::compile()`, then in the rule call `global_query_cache().run_cached_query(...)`
//! and map captures to `Issue`. Keep line-based fallback when the query fails.
//! Priority rules per roadmap: assertion_quality, assertion_intent, error_coverage,
//! boundary_conditions, async_patterns, mock_abuse, test_isolation, trivial_assertion, naming_quality.

use std::collections::HashMap;
use std::sync::Mutex;
use tree_sitter::{Language, Query, QueryCursor, StreamingIterator, Tree};

/// Cache of compiled queries per query_id. Compile once per language, reuse per file.
pub struct QueryCache {
    ts: Mutex<HashMap<QueryId, Query>>,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum QueryId {
    /// console.log / console.debug / console.warn in call_expression
    ConsoleCall,
    /// debugger statement
    DebuggerStatement,
    /// it.only / test.only / describe.only (call with .only member)
    FocusedTestOnly,
}

/// One capture from a query match: capture name and the node's byte range + text.
#[derive(Debug)]
pub struct QueryCaptureInfo {
    pub name: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_point: (usize, usize),
    pub end_point: (usize, usize),
    pub text: String,
}

impl QueryCache {
    pub fn new() -> Self {
        Self {
            ts: Mutex::new(HashMap::new()),
        }
    }

    fn compile(lang: &Language, query_id: QueryId) -> Result<Query, tree_sitter::QueryError> {
        let source = match query_id {
            QueryId::ConsoleCall => {
                // Match call_expression where function is member_expression with object "console"
                r#"
                (call_expression
                  function: (member_expression
                    object: (identifier) @obj
                    property: (property_identifier) @prop))
                "#
            }
            QueryId::DebuggerStatement => "(debugger_statement) @stmt",
            QueryId::FocusedTestOnly => {
                // Match call_expression where function is member_expression: it.only, test.only, describe.only
                r#"
                (call_expression
                  function: (member_expression
                    object: (identifier) @obj
                    property: (property_identifier) @prop))
                "#
            }
        };
        Query::new(lang, source)
    }

    /// Run a cached query on the tree. Returns list of captures per match.
    /// For ConsoleCall and FocusedTestOnly, filter by node text in the caller (e.g. obj == "console", prop in ["log","debug","warn"]).
    pub fn run_cached_query(
        &self,
        source: &str,
        tree: &Tree,
        lang: &Language,
        query_id: QueryId,
    ) -> Result<Vec<Vec<QueryCaptureInfo>>, tree_sitter::QueryError> {
        let mut guard = self.ts.lock().expect("query cache lock");
        if let std::collections::hash_map::Entry::Vacant(e) = guard.entry(query_id) {
            let q = Self::compile(lang, query_id)?;
            e.insert(q);
        }
        let query = guard.get(&query_id).unwrap();
        let mut cursor = QueryCursor::new();
        let mut results = Vec::new();
        let mut matches = cursor.matches(query, tree.root_node(), source.as_bytes());
        while let Some(qm) = matches.next() {
            let mut caps = Vec::new();
            for cap in qm.captures {
                let node = cap.node;
                let start = node.start_byte();
                let end = node.end_byte();
                let s = node.start_position();
                let e = node.end_position();
                let text = source.get(start..end).unwrap_or("").to_string();
                caps.push(QueryCaptureInfo {
                    name: query.capture_names()[cap.index as usize].to_string(),
                    start_byte: start,
                    end_byte: end,
                    start_point: (s.row + 1, s.column + 1),
                    end_point: (e.row + 1, e.column + 1),
                    text,
                });
            }
            results.push(caps);
        }
        Ok(results)
    }
}

impl Default for QueryCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Global query cache (compile once per process).
pub fn global_query_cache() -> &'static QueryCache {
    use std::sync::OnceLock;
    static CACHE: OnceLock<QueryCache> = OnceLock::new();
    CACHE.get_or_init(QueryCache::new)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::TypeScriptParser;

    #[test]
    fn cache_compiles_console_query() {
        let cache = QueryCache::new();
        let lang = TypeScriptParser::language();
        let mut parser = TypeScriptParser::new().unwrap();
        let source = "console.log('x');";
        let tree = parser.parse(source).unwrap();
        let results = cache
            .run_cached_query(source, &tree, &lang, QueryId::ConsoleCall)
            .unwrap();
        assert!(!results.is_empty());
        let caps = &results[0];
        let obj = caps.iter().find(|c| c.name == "obj").unwrap();
        let prop = caps.iter().find(|c| c.name == "prop").unwrap();
        assert_eq!(obj.text, "console");
        assert_eq!(prop.text, "log");
    }

    #[test]
    fn cache_compiles_debugger_query() {
        let cache = QueryCache::new();
        let lang = TypeScriptParser::language();
        let mut parser = TypeScriptParser::new().unwrap();
        let source = "function f() { debugger; }";
        let tree = parser.parse(source).unwrap();
        let results = cache
            .run_cached_query(source, &tree, &lang, QueryId::DebuggerStatement)
            .unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn cache_compiles_focused_test_query() {
        let cache = QueryCache::new();
        let lang = TypeScriptParser::language();
        let mut parser = TypeScriptParser::new().unwrap();
        let source = "it.only('test', () => { expect(1).toBe(1); });";
        let tree = parser.parse(source).unwrap();
        let results = cache
            .run_cached_query(source, &tree, &lang, QueryId::FocusedTestOnly)
            .unwrap();
        assert!(!results.is_empty());
        let caps = &results[0];
        let obj = caps.iter().find(|c| c.name == "obj").unwrap();
        let prop = caps.iter().find(|c| c.name == "prop").unwrap();
        assert_eq!(obj.text, "it");
        assert_eq!(prop.text, "only");
    }
}
