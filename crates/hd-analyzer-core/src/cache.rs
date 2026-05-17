use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::driver::DirectoryListing;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheFreshness {
    Fresh,
    Partial,
    Stale,
    Invalidating,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryCacheEntry {
    pub path: PathBuf,
    pub config_fingerprint: String,
    pub listing: DirectoryListing,
    pub freshness: CacheFreshness,
    pub active_jobs: Vec<String>,
    pub generation: u64,
    pub updated_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    path: PathBuf,
    config_fingerprint: String,
}

#[derive(Debug, Default)]
pub struct DirectoryCache {
    entries: HashMap<CacheKey, DirectoryCacheEntry>,
    next_generation: u64,
}

impl DirectoryCache {
    pub fn get(&self, path: &Path, config_fingerprint: &str) -> Option<DirectoryCacheEntry> {
        self.entries
            .get(&CacheKey {
                path: path.to_path_buf(),
                config_fingerprint: config_fingerprint.to_string(),
            })
            .cloned()
    }

    pub fn upsert(
        &mut self,
        mut listing: DirectoryListing,
        freshness: CacheFreshness,
    ) -> DirectoryCacheEntry {
        self.next_generation = self.next_generation.saturating_add(1);
        listing.generation = self.next_generation;
        let entry = DirectoryCacheEntry {
            path: listing.path.clone(),
            config_fingerprint: listing.config_fingerprint.clone(),
            listing,
            freshness,
            active_jobs: Vec::new(),
            generation: self.next_generation,
            updated_at: SystemTime::now(),
        };
        self.entries.insert(
            CacheKey {
                path: entry.path.clone(),
                config_fingerprint: entry.config_fingerprint.clone(),
            },
            entry.clone(),
        );
        entry
    }

    pub fn mark_stale(&mut self, path: &Path, descendants: bool) -> Vec<PathBuf> {
        let mut invalidated = Vec::new();
        for entry in self.entries.values_mut() {
            let affected = if descendants {
                entry.path.starts_with(path)
            } else {
                entry.path == path
            };
            if affected {
                entry.freshness = CacheFreshness::Stale;
                entry.listing.state = crate::driver::NodeState::Stale;
                invalidated.push(entry.path.clone());
            }
        }
        invalidated.sort();
        invalidated.dedup();
        invalidated
    }
}

#[cfg(test)]
mod tests {
    use crate::driver::{DirectoryListing, NodeState};

    use super::*;

    fn listing(path: &str) -> DirectoryListing {
        DirectoryListing {
            path: PathBuf::from(path),
            config_fingerprint: "a".to_string(),
            children: Vec::new(),
            total_visible_size: 0,
            total_measured_size: 0,
            state: NodeState::Complete,
            loaded_depth: 1,
            has_more_depth: false,
            issues: Vec::new(),
            generation: 0,
        }
    }

    #[test]
    fn upsert_bumps_generation() {
        let mut cache = DirectoryCache::default();
        let first = cache.upsert(listing("/tmp"), CacheFreshness::Fresh);
        let second = cache.upsert(listing("/tmp/a"), CacheFreshness::Fresh);

        assert!(second.generation > first.generation);
    }

    #[test]
    fn descendant_invalidation_preserves_unrelated_entries() {
        let mut cache = DirectoryCache::default();
        cache.upsert(listing("/tmp/a"), CacheFreshness::Fresh);
        cache.upsert(listing("/tmp/a/b"), CacheFreshness::Fresh);
        cache.upsert(listing("/tmp/c"), CacheFreshness::Fresh);

        let invalidated = cache.mark_stale(Path::new("/tmp/a"), true);

        assert_eq!(invalidated.len(), 2);
        assert_eq!(
            cache.get(Path::new("/tmp/c"), "a").unwrap().freshness,
            CacheFreshness::Fresh
        );
    }
}
