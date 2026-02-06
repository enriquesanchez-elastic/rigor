//! Integration tests for the watcher public API.
//! Exercises TestWatcher::is_test_file and that watch() can be created for a temp directory.

use rigor::watcher::TestWatcher;
use std::path::Path;

#[test]
fn watcher_is_test_file_accepts_test_suffixes() {
    assert!(TestWatcher::is_test_file(Path::new("foo.test.ts")));
    assert!(TestWatcher::is_test_file(Path::new("foo.test.tsx")));
    assert!(TestWatcher::is_test_file(Path::new("foo.spec.ts")));
    assert!(TestWatcher::is_test_file(Path::new("foo.spec.js")));
    assert!(TestWatcher::is_test_file(Path::new("dir/bar.test.ts")));
}

#[test]
fn watcher_is_test_file_rejects_non_test_files() {
    assert!(!TestWatcher::is_test_file(Path::new("foo.ts")));
    assert!(!TestWatcher::is_test_file(Path::new("foo.js")));
    assert!(!TestWatcher::is_test_file(Path::new("test.ts"))); // no .test. infix
}

#[test]
fn watcher_is_test_file_rejects_node_modules() {
    assert!(!TestWatcher::is_test_file(Path::new(
        "node_modules/foo.test.ts"
    )));
    assert!(!TestWatcher::is_test_file(Path::new(
        "packages/a/node_modules/b.test.ts"
    )));
}

#[test]
fn watcher_watch_temp_dir_succeeds() {
    let dir = tempfile::TempDir::new().unwrap();
    let result = TestWatcher::watch(dir.path());
    assert!(
        result.is_ok(),
        "watch on temp dir should succeed: {:?}",
        result.err()
    );
}
