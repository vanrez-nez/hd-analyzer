use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SizeMeasurementMode {
    LogicalOnly,
    LogicalAndAllocated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveUpdateConfig {
    pub enabled: bool,
    pub throttle_ms: u64,
    pub min_bytes_delta: u64,
    pub max_batch_size: usize,
}

impl Default for LiveUpdateConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            throttle_ms: 250,
            min_bytes_delta: 8_000_000,
            max_batch_size: 128,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanConfig {
    pub requested_depth: usize,
    pub preload_depth: usize,
    pub show_hidden: bool,
    pub expand_above_bytes: Option<u64>,
    pub min_visible_folder_bytes: Option<u64>,
    pub stay_on_filesystem: bool,
    pub follow_symlinks: bool,
    pub dedupe_hard_links: bool,
    pub size_measurement_mode: SizeMeasurementMode,
    pub live_updates: LiveUpdateConfig,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            requested_depth: 1,
            preload_depth: 1,
            show_hidden: false,
            expand_above_bytes: Some(1_000_000_000),
            min_visible_folder_bytes: Some(100_000_000),
            stay_on_filesystem: true,
            follow_symlinks: false,
            dedupe_hard_links: true,
            size_measurement_mode: SizeMeasurementMode::LogicalOnly,
            live_updates: LiveUpdateConfig::default(),
        }
    }
}

impl ScanConfig {
    pub fn normalized(mut self) -> Self {
        if self.preload_depth > self.requested_depth {
            self.preload_depth = self.requested_depth;
        }
        self
    }

    pub fn fingerprint(&self) -> String {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    pub fn should_expand_folder(&self, size: u64, depth: usize) -> bool {
        depth < self.requested_depth
            && self
                .expand_above_bytes
                .is_some_and(|threshold| size > threshold)
    }

    pub fn should_show_folder(&self, size: u64) -> bool {
        self.min_visible_folder_bytes
            .is_none_or(|threshold| size >= threshold)
    }

    pub fn is_hidden_name(&self, name: &str) -> bool {
        !self.show_hidden && name.starts_with('.') && name != "." && name != ".."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_caps_preload_depth() {
        let config = ScanConfig {
            requested_depth: 2,
            preload_depth: 10,
            ..ScanConfig::default()
        }
        .normalized();

        assert_eq!(config.preload_depth, 2);
    }

    #[test]
    fn fingerprint_changes_when_visibility_changes() {
        let hidden = ScanConfig::default();
        let visible = ScanConfig {
            show_hidden: true,
            ..ScanConfig::default()
        };

        assert_ne!(hidden.fingerprint(), visible.fingerprint());
    }

    #[test]
    fn fingerprint_changes_when_hard_link_policy_changes() {
        let deduped = ScanConfig::default();
        let counted_per_path = ScanConfig {
            dedupe_hard_links: false,
            ..ScanConfig::default()
        };

        assert_ne!(deduped.fingerprint(), counted_per_path.fingerprint());
    }

    #[test]
    fn fingerprint_changes_when_size_measurement_mode_changes() {
        let logical_only = ScanConfig::default();
        let allocated = ScanConfig {
            size_measurement_mode: SizeMeasurementMode::LogicalAndAllocated,
            ..ScanConfig::default()
        };

        assert_ne!(logical_only.fingerprint(), allocated.fingerprint());
    }

    #[test]
    fn fingerprint_changes_when_live_update_policy_changes() {
        let default_updates = ScanConfig::default();
        let slower_updates = ScanConfig {
            live_updates: LiveUpdateConfig {
                throttle_ms: 1_000,
                ..LiveUpdateConfig::default()
            },
            ..ScanConfig::default()
        };

        assert_ne!(default_updates.fingerprint(), slower_updates.fingerprint());
    }

    #[test]
    fn threshold_rules_are_strict_for_expand_and_inclusive_for_visibility() {
        let config = ScanConfig {
            expand_above_bytes: Some(10),
            min_visible_folder_bytes: Some(5),
            requested_depth: 2,
            ..ScanConfig::default()
        };

        assert!(!config.should_expand_folder(10, 0));
        assert!(config.should_expand_folder(11, 0));
        assert!(config.should_show_folder(5));
        assert!(!config.should_show_folder(4));
    }
}
