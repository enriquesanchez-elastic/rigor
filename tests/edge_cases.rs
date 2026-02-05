//! Edge case tests: degenerate inputs must not panic.

use rigor::analyzer::AnalysisEngine;
use std::io::Write;
use std::path::Path;
use tempfile::NamedTempFile;

fn analyze_path(path: &Path) -> Result<rigor::AnalysisResult, anyhow::Error> {
    let engine = AnalysisEngine::new().without_source_analysis();
    engine.analyze(path, None)
}

#[test]
fn empty_file_no_panic() {
    let mut file = NamedTempFile::with_suffix(".test.ts").unwrap();
    file.write_all(b"").unwrap();
    file.flush().unwrap();
    let result = analyze_path(file.path());
    assert!(result.is_ok());
    let r = result.unwrap();
    assert_eq!(r.stats.total_tests, 0);
}

#[test]
fn not_typescript_no_panic() {
    let mut file = NamedTempFile::with_suffix(".test.ts").unwrap();
    file.write_all(b"hello world").unwrap();
    file.flush().unwrap();
    let result = analyze_path(file.path());
    assert!(result.is_ok());
    let r = result.unwrap();
    assert_eq!(r.stats.total_tests, 0);
}

#[test]
fn only_comments_no_crash() {
    let mut file = NamedTempFile::with_suffix(".test.ts").unwrap();
    file.write_all(b"// nothing here\n/* or here */").unwrap();
    file.flush().unwrap();
    let result = analyze_path(file.path());
    assert!(result.is_ok());
    let r = result.unwrap();
    assert_eq!(r.stats.total_tests, 0);
}

#[test]
fn syntax_error_handled_gracefully() {
    let mut file = NamedTempFile::with_suffix(".test.ts").unwrap();
    file.write_all(b"function {{{ broken").unwrap();
    file.flush().unwrap();
    let result = analyze_path(file.path());
    assert!(result.is_err() || result.is_ok());
    if let Ok(r) = result {
        assert_eq!(r.stats.total_tests, 0);
    }
}

#[test]
fn no_describe_block_extracts_test() {
    let mut file = NamedTempFile::with_suffix(".test.ts").unwrap();
    file.write_all(b"it('test', () => { expect(1).toBe(1); });").unwrap();
    file.flush().unwrap();
    let result = analyze_path(file.path());
    assert!(result.is_ok());
    let r = result.unwrap();
    assert!(r.stats.total_tests >= 1);
}

#[test]
fn deeply_nested_describes() {
    let mut content = String::new();
    for i in 0..10 {
        content.push_str(&format!("describe('level{}', () => {{\n", i));
    }
    content.push_str("it('deep test', () => { expect(1).toBe(1); });\n");
    for _ in 0..10 {
        content.push_str("});\n");
    }
    let mut file = NamedTempFile::with_suffix(".test.ts").unwrap();
    file.write_all(content.as_bytes()).unwrap();
    file.flush().unwrap();
    let result = analyze_path(file.path());
    assert!(result.is_ok());
    let r = result.unwrap();
    assert!(r.stats.total_tests >= 1);
}

#[test]
fn utf8_identifiers_no_crash() {
    let mut file = NamedTempFile::with_suffix(".test.ts").unwrap();
    file.write_all("describe('テスト', () => { it('works', () => { expect(1).toBe(1); }); });".as_bytes())
        .unwrap();
    file.flush().unwrap();
    let result = analyze_path(file.path());
    assert!(result.is_ok());
}

#[test]
fn file_with_bom_parses() {
    let mut file = NamedTempFile::with_suffix(".test.ts").unwrap();
    file.write_all(b"\xEF\xBB\xBFdescribe('x', () => { it('t', () => {}); });")
        .unwrap();
    file.flush().unwrap();
    let result = analyze_path(file.path());
    assert!(result.is_ok());
    let r = result.unwrap();
    assert!(r.stats.total_tests >= 1);
}

#[test]
fn large_file_completes() {
    let mut content = String::from("describe('big', () => {\n");
    for i in 0..500 {
        content.push_str(&format!(
            "it('test {}', () => {{ expect({}).toBe({}); }});\n",
            i, i, i
        ));
    }
    content.push_str("});");
    let mut file = NamedTempFile::with_suffix(".test.ts").unwrap();
    file.write_all(content.as_bytes()).unwrap();
    file.flush().unwrap();
    let result = analyze_path(file.path());
    assert!(result.is_ok());
    let r = result.unwrap();
    assert!(r.stats.total_tests >= 500);
}
