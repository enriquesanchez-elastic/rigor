//! Integration tests for the cache public API.
//! Exercises cache set/get, cleanup (eviction), and save/load from outside the crate.

use rigor::cache::AnalysisCache;
use rigor::{Score, ScoreBreakdown, TestFramework, TestStats, TestType};
use std::path::{Path, PathBuf};

fn make_result(path: &str) -> rigor::AnalysisResult {
    rigor::AnalysisResult {
        file_path: PathBuf::from(path),
        score: Score::new(85),
        breakdown: ScoreBreakdown {
            assertion_quality: 20,
            error_coverage: 18,
            boundary_conditions: 15,
            test_isolation: 17,
            input_variety: 15,
            ai_smells: 25,
        },
        transparent_breakdown: None,
        test_scores: None,
        issues: vec![],
        stats: TestStats {
            total_tests: 3,
            ..TestStats::default()
        },
        framework: TestFramework::Jest,
        test_type: TestType::Unit,
        source_file: None,
    }
}

#[test]
fn cache_cleanup_evicts_entries_not_in_existing_files() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut cache = AnalysisCache::new(dir.path());

    cache.set(
        Path::new("a.test.ts"),
        "content a",
        None,
        make_result("a.test.ts"),
    );
    cache.set(
        Path::new("b.test.ts"),
        "content b",
        None,
        make_result("b.test.ts"),
    );
    assert_eq!(cache.stats().entries, 2);

    cache.cleanup(&[PathBuf::from("a.test.ts")]);

    assert_eq!(cache.stats().entries, 1);
    assert!(cache
        .get(Path::new("a.test.ts"), "content a", None)
        .is_some());
    assert!(cache
        .get(Path::new("b.test.ts"), "content b", None)
        .is_none());
}

#[test]
fn cache_save_and_reload_persists_entries() {
    let dir = tempfile::TempDir::new().unwrap();

    {
        let mut cache = AnalysisCache::new(dir.path());
        cache.set(
            Path::new("persist.test.ts"),
            "const x = 1;",
            None,
            make_result("persist.test.ts"),
        );
        cache.save().unwrap();
    }

    {
        let cache = AnalysisCache::new(dir.path());
        let cached = cache.get(Path::new("persist.test.ts"), "const x = 1;", None);
        assert!(cached.is_some(), "cache should persist after save/load");
        assert_eq!(cached.unwrap().score.value, 85);
    }
}
