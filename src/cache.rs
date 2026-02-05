//! Analysis result caching for performance
//!
//! Caches analysis results based on file content hash to avoid re-analyzing unchanged files.

use crate::AnalysisResult;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const CACHE_VERSION: u32 = 1;
const CACHE_FILENAME: &str = ".rigor-cache.json";

/// Cache entry for a single file
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    /// SHA256 hash of the file content
    content_hash: String,
    /// SHA256 hash of the source file content (if any)
    source_hash: Option<String>,
    /// Cached analysis result
    result: AnalysisResult,
    /// Timestamp when cached
    cached_at: u64,
}

/// Analysis cache
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheData {
    /// Cache format version
    version: u32,
    /// Map of file path to cache entry
    entries: HashMap<String, CacheEntry>,
}

impl Default for CacheData {
    fn default() -> Self {
        Self {
            version: CACHE_VERSION,
            entries: HashMap::new(),
        }
    }
}

/// Cache manager for analysis results
pub struct AnalysisCache {
    /// Path to the cache file
    cache_path: PathBuf,
    /// Cached data
    data: CacheData,
    /// Whether the cache has been modified
    dirty: bool,
    /// Whether caching is enabled
    enabled: bool,
}

impl AnalysisCache {
    /// Create a new cache manager
    pub fn new(project_root: &Path) -> Self {
        let cache_path = project_root.join(CACHE_FILENAME);
        let data = Self::load_cache(&cache_path).unwrap_or_default();

        Self {
            cache_path,
            data,
            dirty: false,
            enabled: true,
        }
    }

    /// Create a disabled cache (no-op)
    pub fn disabled() -> Self {
        Self {
            cache_path: PathBuf::new(),
            data: CacheData::default(),
            dirty: false,
            enabled: false,
        }
    }

    /// Load cache from disk
    fn load_cache(path: &Path) -> Option<CacheData> {
        let content = fs::read_to_string(path).ok()?;
        let data: CacheData = serde_json::from_str(&content).ok()?;

        // Check version compatibility
        if data.version != CACHE_VERSION {
            return None;
        }

        Some(data)
    }

    /// Save cache to disk
    pub fn save(&self) -> Result<()> {
        if !self.enabled || !self.dirty {
            return Ok(());
        }

        let content =
            serde_json::to_string_pretty(&self.data).context("Failed to serialize cache")?;
        fs::write(&self.cache_path, content)
            .with_context(|| format!("Failed to write cache to {}", self.cache_path.display()))?;

        Ok(())
    }

    /// Compute SHA256 hash of file content
    fn hash_content(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Get cached result if still valid
    pub fn get(
        &self,
        test_path: &Path,
        test_content: &str,
        source_content: Option<&str>,
    ) -> Option<AnalysisResult> {
        if !self.enabled {
            return None;
        }

        let key = test_path.to_string_lossy().to_string();
        let entry = self.data.entries.get(&key)?;

        // Check if test file hash matches
        let current_hash = Self::hash_content(test_content);
        if entry.content_hash != current_hash {
            return None;
        }

        // Check if source file hash matches (if applicable)
        match (source_content, &entry.source_hash) {
            (Some(content), Some(cached_hash)) => {
                let current_source_hash = Self::hash_content(content);
                if &current_source_hash != cached_hash {
                    return None;
                }
            }
            (None, None) => {}
            _ => return None, // Source presence changed
        }

        Some(entry.result.clone())
    }

    /// Store analysis result in cache
    pub fn set(
        &mut self,
        test_path: &Path,
        test_content: &str,
        source_content: Option<&str>,
        result: AnalysisResult,
    ) {
        if !self.enabled {
            return;
        }

        let key = test_path.to_string_lossy().to_string();
        let entry = CacheEntry {
            content_hash: Self::hash_content(test_content),
            source_hash: source_content.map(Self::hash_content),
            result,
            cached_at: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        };

        self.data.entries.insert(key, entry);
        self.dirty = true;
    }

    /// Remove stale entries for files that no longer exist
    pub fn cleanup(&mut self, existing_files: &[PathBuf]) {
        if !self.enabled {
            return;
        }

        let existing_set: std::collections::HashSet<String> = existing_files
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        self.data.entries.retain(|k, _| existing_set.contains(k));
        self.dirty = true;
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            entries: self.data.entries.len(),
            enabled: self.enabled,
        }
    }

    /// Clear all cached entries
    pub fn clear(&mut self) {
        self.data.entries.clear();
        self.dirty = true;
    }
}

/// Cache statistics
#[derive(Debug)]
pub struct CacheStats {
    pub entries: usize,
    pub enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Score, ScoreBreakdown, TestFramework, TestStats, TestType};

    fn make_result(path: &str) -> AnalysisResult {
        AnalysisResult {
            file_path: PathBuf::from(path),
            score: Score::new(85),
            breakdown: ScoreBreakdown {
                assertion_quality: 20,
                error_coverage: 18,
                boundary_conditions: 15,
                test_isolation: 17,
                input_variety: 15,
            },
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
    fn test_hash_content() {
        let hash1 = AnalysisCache::hash_content("test content");
        let hash2 = AnalysisCache::hash_content("test content");
        let hash3 = AnalysisCache::hash_content("different content");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_cache_disabled() {
        let cache = AnalysisCache::disabled();
        assert!(!cache.enabled);
        assert!(cache.get(Path::new("test.ts"), "content", None).is_none());
    }

    #[test]
    fn test_cache_disabled_set_noop() {
        let mut cache = AnalysisCache::disabled();
        let result = make_result("test.ts");
        cache.set(Path::new("test.ts"), "content", None, result);
        // get should still return None since cache is disabled
        assert!(cache.get(Path::new("test.ts"), "content", None).is_none());
        assert!(!cache.dirty, "disabled cache should not become dirty");
    }

    #[test]
    fn test_cache_disabled_stats() {
        let cache = AnalysisCache::disabled();
        let stats = cache.stats();
        assert!(!stats.enabled);
        assert_eq!(stats.entries, 0);
    }

    #[test]
    fn test_cache_roundtrip_hit() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut cache = AnalysisCache::new(dir.path());
        let result = make_result("auth.test.ts");

        cache.set(Path::new("auth.test.ts"), "const x = 1;", None, result.clone());
        assert!(cache.dirty, "cache should be dirty after set");

        let cached = cache.get(Path::new("auth.test.ts"), "const x = 1;", None);
        assert!(cached.is_some(), "cache should hit for same content");
        let cached = cached.unwrap();
        assert_eq!(cached.score.value, 85);
        assert_eq!(cached.stats.total_tests, 3);
    }

    #[test]
    fn test_cache_roundtrip_miss_on_changed_content() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut cache = AnalysisCache::new(dir.path());
        let result = make_result("auth.test.ts");

        cache.set(Path::new("auth.test.ts"), "const x = 1;", None, result);

        let cached = cache.get(Path::new("auth.test.ts"), "const x = 2;", None);
        assert!(cached.is_none(), "cache should miss when content changes");
    }

    #[test]
    fn test_cache_roundtrip_with_source() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut cache = AnalysisCache::new(dir.path());
        let result = make_result("auth.test.ts");

        cache.set(
            Path::new("auth.test.ts"),
            "test content",
            Some("source content"),
            result,
        );

        // Hit with same source
        let cached = cache.get(
            Path::new("auth.test.ts"),
            "test content",
            Some("source content"),
        );
        assert!(cached.is_some(), "should hit with same source content");

        // Miss with changed source
        let cached = cache.get(
            Path::new("auth.test.ts"),
            "test content",
            Some("different source"),
        );
        assert!(cached.is_none(), "should miss when source content changes");

        // Miss when source presence changes (cached with source, queried without)
        let cached = cache.get(Path::new("auth.test.ts"), "test content", None);
        assert!(
            cached.is_none(),
            "should miss when source presence changes"
        );
    }

    #[test]
    fn test_cache_save_and_load() {
        let dir = tempfile::TempDir::new().unwrap();

        // Save a cache entry
        {
            let mut cache = AnalysisCache::new(dir.path());
            let result = make_result("auth.test.ts");
            cache.set(Path::new("auth.test.ts"), "content", None, result);
            cache.save().unwrap();
        }

        // Load and verify
        {
            let cache = AnalysisCache::new(dir.path());
            let cached = cache.get(Path::new("auth.test.ts"), "content", None);
            assert!(cached.is_some(), "cache should persist across save/load");
            assert_eq!(cached.unwrap().score.value, 85);
        }
    }

    #[test]
    fn test_cache_clear() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut cache = AnalysisCache::new(dir.path());
        let result = make_result("auth.test.ts");
        cache.set(Path::new("auth.test.ts"), "content", None, result);

        assert_eq!(cache.stats().entries, 1);
        cache.clear();
        assert_eq!(cache.stats().entries, 0);
        assert!(cache.get(Path::new("auth.test.ts"), "content", None).is_none());
    }

    #[test]
    fn test_cache_cleanup() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut cache = AnalysisCache::new(dir.path());
        cache.set(
            Path::new("a.test.ts"),
            "a",
            None,
            make_result("a.test.ts"),
        );
        cache.set(
            Path::new("b.test.ts"),
            "b",
            None,
            make_result("b.test.ts"),
        );
        assert_eq!(cache.stats().entries, 2);

        // Only a.test.ts still exists
        cache.cleanup(&[PathBuf::from("a.test.ts")]);
        assert_eq!(cache.stats().entries, 1);
        assert!(cache.get(Path::new("a.test.ts"), "a", None).is_some());
        assert!(cache.get(Path::new("b.test.ts"), "b", None).is_none());
    }

    #[test]
    fn test_cache_disabled_cleanup_noop() {
        let mut cache = AnalysisCache::disabled();
        // Should not panic
        cache.cleanup(&[PathBuf::from("a.test.ts")]);
        assert!(!cache.dirty);
    }
}
