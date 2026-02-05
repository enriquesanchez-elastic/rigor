//! File system watcher for watch mode

use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

const DEBOUNCE_MS: u64 = 300;

/// Watches a directory for test file changes and emits paths on a channel
pub struct TestWatcher {
    _watcher: RecommendedWatcher,
    receiver: Receiver<notify::Result<notify::Event>>,
}

fn is_create_or_modify(kind: &EventKind) -> bool {
    matches!(kind, EventKind::Create(_) | EventKind::Modify(_))
}

impl TestWatcher {
    /// Start watching the given path (file or directory)
    pub fn watch(path: &Path) -> notify::Result<Self> {
        let (tx, rx) = channel();
        let mut watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            Config::default().with_poll_interval(Duration::from_millis(DEBOUNCE_MS)),
        )?;

        if path.is_dir() {
            watcher.watch(path, RecursiveMode::Recursive)?;
        } else if let Some(parent) = path.parent() {
            watcher.watch(parent, RecursiveMode::Recursive)?;
        }

        Ok(Self {
            _watcher: watcher,
            receiver: rx,
        })
    }

    /// Check if the path is a test file we care about
    pub fn is_test_file(p: &Path) -> bool {
        let name = match p.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => return false,
        };
        if p.components().any(|c| c.as_os_str() == "node_modules") {
            return false;
        }
        [
            ".test.ts",
            ".test.tsx",
            ".spec.ts",
            ".spec.tsx",
            ".test.js",
            ".test.jsx",
            ".spec.js",
            ".spec.jsx",
        ]
        .iter()
        .any(|suffix| name.ends_with(suffix))
    }

    /// Collect test paths from an event
    fn paths_from_event(event: &notify::Event) -> Vec<PathBuf> {
        if !is_create_or_modify(&event.kind) {
            return vec![];
        }
        event
            .paths
            .iter()
            .filter(|p| Self::is_test_file(p))
            .cloned()
            .collect()
    }

    /// Wait for the next batch of changes (debounced). Blocks until at least one change, then drains for DEBOUNCE_MS.
    pub fn next_changes(&self) -> Vec<PathBuf> {
        let mut all = std::collections::HashSet::new();

        // Wait for first event (with timeout so we can react to shutdown)
        match self.receiver.recv_timeout(Duration::from_secs(3600)) {
            Ok(Ok(event)) => {
                for p in Self::paths_from_event(&event) {
                    all.insert(p);
                }
            }
            Ok(Err(_)) => return vec![],
            Err(_) => return vec![],
        }

        // Debounce: collect further events for a short time
        std::thread::sleep(Duration::from_millis(DEBOUNCE_MS));
        while let Ok(ev) = self.receiver.try_recv() {
            if let Ok(event) = ev {
                for p in Self::paths_from_event(&event) {
                    all.insert(p);
                }
            }
        }

        all.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_is_test_file_ts() {
        assert!(TestWatcher::is_test_file(Path::new("auth.test.ts")));
        assert!(TestWatcher::is_test_file(Path::new("auth.spec.ts")));
        assert!(TestWatcher::is_test_file(Path::new("Button.test.tsx")));
        assert!(TestWatcher::is_test_file(Path::new("Button.spec.tsx")));
    }

    #[test]
    fn test_is_test_file_js() {
        assert!(TestWatcher::is_test_file(Path::new("auth.test.js")));
        assert!(TestWatcher::is_test_file(Path::new("auth.spec.js")));
        assert!(TestWatcher::is_test_file(Path::new("Button.test.jsx")));
        assert!(TestWatcher::is_test_file(Path::new("Button.spec.jsx")));
    }

    #[test]
    fn test_is_test_file_non_test() {
        assert!(!TestWatcher::is_test_file(Path::new("auth.ts")));
        assert!(!TestWatcher::is_test_file(Path::new("index.js")));
        assert!(!TestWatcher::is_test_file(Path::new("README.md")));
        assert!(!TestWatcher::is_test_file(Path::new("package.json")));
    }

    #[test]
    fn test_is_test_file_node_modules_excluded() {
        assert!(!TestWatcher::is_test_file(Path::new(
            "node_modules/jest/test.test.ts"
        )));
        assert!(!TestWatcher::is_test_file(Path::new(
            "project/node_modules/lib/auth.test.js"
        )));
    }

    #[test]
    fn test_is_test_file_nested_path() {
        assert!(TestWatcher::is_test_file(Path::new(
            "src/auth/login.test.ts"
        )));
        assert!(TestWatcher::is_test_file(Path::new(
            "tests/__tests__/Button.spec.tsx"
        )));
    }

    #[test]
    fn test_is_test_file_no_name() {
        // Path with no file name
        assert!(!TestWatcher::is_test_file(Path::new("")));
    }

    #[test]
    fn test_is_create_or_modify() {
        use notify::event::{CreateKind, ModifyKind, RemoveKind};
        assert!(is_create_or_modify(&EventKind::Create(CreateKind::File)));
        assert!(is_create_or_modify(&EventKind::Modify(ModifyKind::Data(
            notify::event::DataChange::Content
        ))));
        assert!(!is_create_or_modify(&EventKind::Remove(RemoveKind::File)));
    }

    #[test]
    fn test_paths_from_event_filters_test_files() {
        use notify::event::{CreateKind, RemoveKind};

        // Create event with mixed paths
        let event = notify::Event {
            kind: EventKind::Create(CreateKind::File),
            paths: vec![
                PathBuf::from("src/auth.test.ts"),
                PathBuf::from("src/auth.ts"),
                PathBuf::from("src/cart.spec.js"),
            ],
            attrs: Default::default(),
        };

        let paths = TestWatcher::paths_from_event(&event);
        assert_eq!(paths.len(), 2);
        assert!(paths.contains(&PathBuf::from("src/auth.test.ts")));
        assert!(paths.contains(&PathBuf::from("src/cart.spec.js")));

        // Remove event should return empty
        let remove_event = notify::Event {
            kind: EventKind::Remove(RemoveKind::File),
            paths: vec![PathBuf::from("src/auth.test.ts")],
            attrs: Default::default(),
        };
        let paths = TestWatcher::paths_from_event(&remove_event);
        assert!(paths.is_empty());
    }

    #[test]
    fn test_watch_creates_watcher() {
        // Verify that TestWatcher::watch succeeds and doesn't panic
        let dir = tempfile::TempDir::new().unwrap();
        let watcher = TestWatcher::watch(dir.path());
        assert!(watcher.is_ok(), "watch should succeed on a temp dir");
        // Note: next_changes() blocks for up to 3600s, so we don't call it in unit tests.
        // The watcher integration is best tested manually or in a dedicated integration test.
    }

    #[test]
    fn test_watch_single_file_parent() {
        // When watching a single file, it should watch the parent directory
        let dir = tempfile::TempDir::new().unwrap();
        let file = dir.path().join("test.test.ts");
        std::fs::write(&file, "test").unwrap();
        let watcher = TestWatcher::watch(&file);
        assert!(watcher.is_ok(), "watch should succeed for a single file");
    }
}
