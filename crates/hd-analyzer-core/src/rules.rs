use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};

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
