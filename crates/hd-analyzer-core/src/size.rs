use std::collections::HashSet;
use std::fs::Metadata;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SizeIdentity {
    pub platform: String,
    pub dev: Option<u64>,
    pub ino: Option<u64>,
    pub file_index: Option<u64>,
    pub volume_serial: Option<u64>,
}

impl SizeIdentity {
    #[cfg(unix)]
    fn from_metadata(metadata: &Metadata) -> Self {
        use std::os::unix::fs::MetadataExt;

        Self {
            platform: "unix".to_string(),
            dev: Some(metadata.dev()),
            ino: Some(metadata.ino()),
            file_index: None,
            volume_serial: None,
        }
    }

    #[cfg(not(unix))]
    fn from_metadata(_metadata: &Metadata) -> Self {
        Self {
            platform: std::env::consts::OS.to_string(),
            dev: None,
            ino: None,
            file_index: None,
            volume_serial: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeasurementMethod {
    UnixBlocks,
    WindowsCompressedSize,
    LogicalFallback,
    DirectoryAggregate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SizeMeasurement {
    pub allocated_bytes: u64,
    pub logical_bytes: u64,
    pub measurement_method: MeasurementMethod,
    pub identity: Option<SizeIdentity>,
    pub deduped: bool,
}

#[derive(Debug, Default)]
pub struct HardLinkDedupe {
    seen: HashSet<SizeIdentity>,
}

impl HardLinkDedupe {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn measure_file(&mut self, path: &Path, metadata: &Metadata) -> SizeMeasurement {
        let identity = hard_link_identity(metadata);
        let deduped = identity
            .as_ref()
            .is_some_and(|identity| !self.seen.insert(identity.clone()));
        let (allocated_bytes, measurement_method) = if deduped {
            (0, platform_allocated_size(path, metadata).1)
        } else {
            platform_allocated_size(path, metadata)
        };

        SizeMeasurement {
            allocated_bytes,
            logical_bytes: metadata.len(),
            measurement_method,
            identity,
            deduped,
        }
    }
}

pub fn directory_measurement(allocated_bytes: u64, logical_bytes: u64) -> SizeMeasurement {
    SizeMeasurement {
        allocated_bytes,
        logical_bytes,
        measurement_method: MeasurementMethod::DirectoryAggregate,
        identity: None,
        deduped: false,
    }
}

fn hard_link_identity(metadata: &Metadata) -> Option<SizeIdentity> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() > 1 {
            return Some(SizeIdentity::from_metadata(metadata));
        }
    }

    let _ = metadata;
    None
}

pub fn platform_allocated_size(path: &Path, metadata: &Metadata) -> (u64, MeasurementMethod) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let allocated = metadata.blocks().saturating_mul(512);
        if allocated > 0 {
            return (allocated, MeasurementMethod::UnixBlocks);
        }
    }

    #[cfg(windows)]
    {
        if let Some(size) = windows_allocated_size(path) {
            return (size, MeasurementMethod::WindowsCompressedSize);
        }
    }

    let _ = path;
    (metadata.len(), MeasurementMethod::LogicalFallback)
}

#[cfg(windows)]
fn windows_allocated_size(path: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;

    use windows::Win32::Storage::FileSystem::GetCompressedFileSizeW;
    use windows::core::PCWSTR;

    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);
    let mut high = 0u32;
    let low = unsafe { GetCompressedFileSizeW(PCWSTR(wide.as_ptr()), Some(&mut high)) };
    if low == u32::MAX && std::io::Error::last_os_error().raw_os_error().is_some() {
        return None;
    }
    Some(((high as u64) << 32) | low as u64)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn measurement_uses_nonzero_size_for_normal_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("file.txt");
        std::fs::write(&path, b"hello").unwrap();

        let metadata = std::fs::metadata(&path).unwrap();
        let mut dedupe = HardLinkDedupe::new();
        let measurement = dedupe.measure_file(&path, &metadata);

        assert!(measurement.allocated_bytes > 0);
    }

    #[cfg(unix)]
    #[test]
    fn hard_linked_file_counts_allocated_size_once() {
        let dir = tempdir().unwrap();
        let first = dir.path().join("first.bin");
        let second = dir.path().join("second.bin");
        std::fs::write(&first, b"linked").unwrap();
        std::fs::hard_link(&first, &second).unwrap();

        let mut dedupe = HardLinkDedupe::new();
        let first_size = dedupe.measure_file(&first, &std::fs::metadata(&first).unwrap());
        let second_size = dedupe.measure_file(&second, &std::fs::metadata(&second).unwrap());

        assert!(first_size.allocated_bytes > 0);
        assert_eq!(second_size.allocated_bytes, 0);
    }
}
