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
    matches!(
        kind,
        EventKind::Create(_) | EventKind::Modify(_)
    )
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
            ".test.ts", ".test.tsx", ".spec.ts", ".spec.tsx",
            ".test.js", ".test.jsx", ".spec.js", ".spec.jsx",
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
