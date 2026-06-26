#![no_main]
//! Fuzz the in-memory analysis path with arbitrary source bytes.
//!
//! Rigor parses untrusted TypeScript supplied by users (and, via the LSP, on
//! every keystroke). `analyze_source` must never panic — it should return an
//! error or an empty result, but stay alive. This target feeds it raw input.
//!
//! Run with:
//!     cargo install cargo-fuzz
//!     cargo +nightly fuzz run analyze_source

use libfuzzer_sys::fuzz_target;
use rigor::analyzer::AnalysisEngine;
use std::path::Path;

fuzz_target!(|data: &[u8]| {
    // tree-sitter operates on &str; skip non-UTF8 (not a panic condition).
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };

    let engine = AnalysisEngine::new().without_source_analysis();

    // Exercise both the .ts and .tsx parser paths — extension drives grammar.
    let _ = engine.analyze_source(source, Path::new("fuzz.test.ts"), None);
    let _ = engine.analyze_source(source, Path::new("fuzz.test.tsx"), None);
});
