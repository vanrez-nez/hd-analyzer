use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::driver::{DirectoryListing, DirectorySizeSummary, PreciseDirectoryScan};

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
    summaries: HashMap<CacheKey, DirectorySizeSummary>,
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

    pub fn get_summary(
        &self,
        path: &Path,
        config_fingerprint: &str,
    ) -> Option<DirectorySizeSummary> {
        self.summaries
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

    pub fn upsert_precise(
        &mut self,
        precise: PreciseDirectoryScan,
        freshness: CacheFreshness,
    ) -> DirectoryCacheEntry {
        let root_summary = DirectorySizeSummary {
            path: precise.listing.path.clone(),
            config_fingerprint: precise.listing.config_fingerprint.clone(),
            allocated_size: precise.listing.total_measured_size,
            logical_size: precise.listing.total_logical_size,
            has_visible_children: precise.listing.children.iter().any(|child| child.visible),
            issues: precise.listing.issues.clone(),
        };
        self.summaries.insert(
            CacheKey {
                path: root_summary.path.clone(),
                config_fingerprint: root_summary.config_fingerprint.clone(),
            },
            root_summary,
        );

        for summary in precise.summaries {
            self.summaries.insert(
                CacheKey {
                    path: summary.path.clone(),
                    config_fingerprint: summary.config_fingerprint.clone(),
                },
                summary,
            );
        }

        self.upsert(precise.listing, freshness)
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
        self.summaries.retain(|key, _summary| {
            let affected = if descendants {
                key.path.starts_with(path)
            } else {
                key.path == path
            };
            if affected {
                invalidated.push(key.path.clone());
            }
            !affected
        });
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
            total_logical_size: 0,
            state: NodeState::Complete,
            loaded_depth: 1,
            has_more_depth: false,
            issues: Vec::new(),
            generation: 0,
        }
    }

    fn summary(path: &str) -> DirectorySizeSummary {
        DirectorySizeSummary {
            path: PathBuf::from(path),
            config_fingerprint: "a".to_string(),
            allocated_size: 10,
            logical_size: 10,
            has_visible_children: false,
            issues: Vec::new(),
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

    #[test]
    fn precise_upsert_stores_directory_size_summaries() {
        let mut cache = DirectoryCache::default();
        cache.upsert_precise(
            PreciseDirectoryScan {
                listing: listing("/tmp"),
                summaries: vec![summary("/tmp/a")],
            },
            CacheFreshness::Fresh,
        );

        assert_eq!(
            cache
                .get_summary(Path::new("/tmp/a"), "a")
                .unwrap()
                .allocated_size,
            10
        );
    }

    #[test]
    fn precise_upsert_stores_root_directory_size_summary() {
        let mut cache = DirectoryCache::default();
        let mut root = listing("/tmp");
        root.total_measured_size = 20;
        root.total_logical_size = 30;
        cache_upsert_precise_root(&mut cache, root);

        let summary = cache.get_summary(Path::new("/tmp"), "a").unwrap();

        assert_eq!(summary.allocated_size, 20);
        assert_eq!(summary.logical_size, 30);
    }

    #[test]
    fn descendant_invalidation_removes_size_summaries() {
        let mut cache = DirectoryCache::default();
        cache.upsert_precise(
            PreciseDirectoryScan {
                listing: listing("/tmp/a"),
                summaries: vec![summary("/tmp/a/b"), summary("/tmp/c")],
            },
            CacheFreshness::Fresh,
        );

        cache.mark_stale(Path::new("/tmp/a"), true);

        assert!(cache.get_summary(Path::new("/tmp/a/b"), "a").is_none());
        assert!(cache.get_summary(Path::new("/tmp/c"), "a").is_some());
    }

    fn cache_upsert_precise_root(cache: &mut DirectoryCache, listing: DirectoryListing) {
        cache.upsert_precise(
            PreciseDirectoryScan {
                listing,
                summaries: Vec::new(),
            },
            CacheFreshness::Fresh,
        );
    }
}
