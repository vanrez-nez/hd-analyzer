use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use sysinfo::Disks;

#[derive(Debug, Clone)]
pub struct Drive {
    pub label: String,
    pub mount_point: PathBuf,
    pub total_space: u64,
    pub available_space: u64,
    pub file_system: String,
}

impl Drive {
    pub fn used_space(&self) -> u64 {
        self.total_space.saturating_sub(self.available_space)
    }
}

pub fn list_drives() -> Result<Vec<Drive>> {
    let disks = Disks::new_with_refreshed_list();
    let mut seen = HashMap::<PathBuf, Drive>::new();

    for disk in disks.list() {
        let mount_point = disk.mount_point().to_path_buf();
        if disk.total_space() == 0 || mount_point.as_os_str().is_empty() {
            continue;
        }

        let file_system = disk.file_system().to_string_lossy().into_owned();
        let disk_name = disk.name().to_string_lossy();
        let label = if disk_name.is_empty() {
            mount_point.display().to_string()
        } else {
            format!("{disk_name} ({})", mount_point.display())
        };

        seen.entry(mount_point.clone()).or_insert(Drive {
            label,
            mount_point,
            total_space: disk.total_space(),
            available_space: disk.available_space(),
            file_system,
        });
    }

    let mut drives: Vec<_> = seen.into_values().collect();
    #[cfg(target_os = "macos")]
    {
        drives = collapse_macos_volume_group_duplicates(drives);
    }
    drives.sort_by(|left, right| left.mount_point.cmp(&right.mount_point));
    if drives.is_empty() {
        anyhow::bail!("No mounted drives found.");
    }
    Ok(drives)
}

#[cfg(target_os = "macos")]
fn collapse_macos_volume_group_duplicates(drives: Vec<Drive>) -> Vec<Drive> {
    let data_mount = Path::new("/System/Volumes/Data");
    let root_mount = Path::new("/");

    let has_data_volume = drives.iter().any(|drive| drive.mount_point == data_mount);
    if !has_data_volume {
        return drives;
    }

    drives
        .into_iter()
        .filter(|drive| drive.mount_point != root_mount)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn used_space_saturates_when_available_exceeds_total() {
        let drive = Drive {
            label: "test".to_string(),
            mount_point: PathBuf::from("/tmp"),
            total_space: 10,
            available_space: 20,
            file_system: "testfs".to_string(),
        };

        assert_eq!(drive.used_space(), 0);
    }
}
