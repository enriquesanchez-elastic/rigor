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
        
        let content = serde_json::to_string_pretty(&self.data)
            .context("Failed to serialize cache")?;
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
    pub fn get(&self, test_path: &Path, test_content: &str, source_content: Option<&str>) -> Option<AnalysisResult> {
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
}
