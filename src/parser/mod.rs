//! Parser module for TypeScript test files

pub mod ast_helpers;
pub mod ignore_directives;
pub mod queries;
pub mod source_file;
pub mod test_file;
pub mod typescript;

pub use ast_helpers::{
    containing_test_body, count_branches, count_branches_with_source, find_assertions_in_body,
    find_call_expressions, is_inside_comment, is_inside_comment_range, is_inside_string_literal,
    is_inside_string_literal_range, node_line_count, node_to_location, CallInfo,
};
pub use ignore_directives::IgnoreDirectives;
pub use queries::{global_query_cache, QueryCache, QueryCaptureInfo, QueryId};
pub use source_file::{
    BoundaryCondition, ExportKind, ExportedItem, FunctionDetails, Mutation, MutationKind,
    Parameter, ReturnStatement, SourceFileParser, ThrowableFunction, ValueShape,
};
pub use test_file::TestFileParser;
pub use typescript::TypeScriptParser;
