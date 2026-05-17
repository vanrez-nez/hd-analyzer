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
    MacOsUrlResource,
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
        self.measure_file_with_policy(path, metadata, true)
    }

    pub fn measure_file_with_policy(
        &mut self,
        path: &Path,
        metadata: &Metadata,
        dedupe_hard_links: bool,
    ) -> SizeMeasurement {
        let identity = hard_link_identity(metadata);
        let deduped = dedupe_hard_links
            && identity
                .as_ref()
                .is_some_and(|identity| !self.seen.insert(identity.clone()));
        let size = platform_file_size(path, metadata);
        let allocated_bytes = if deduped { 0 } else { size.allocated_bytes };

        SizeMeasurement {
            allocated_bytes,
            logical_bytes: size.logical_bytes,
            measurement_method: size.measurement_method,
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

#[derive(Debug, Clone, Copy)]
struct FileSize {
    allocated_bytes: u64,
    logical_bytes: u64,
    measurement_method: MeasurementMethod,
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

fn platform_file_size(path: &Path, metadata: &Metadata) -> FileSize {
    #[cfg(target_os = "macos")]
    {
        if let Some(size) = macos_url_resource_size(path) {
            return size;
        }
    }

    let (allocated_bytes, measurement_method) = platform_allocated_size(path, metadata);
    FileSize {
        allocated_bytes,
        logical_bytes: metadata.len(),
        measurement_method,
    }
}

pub fn platform_allocated_size(path: &Path, metadata: &Metadata) -> (u64, MeasurementMethod) {
    #[cfg(target_os = "macos")]
    {
        if let Some(size) = macos_url_resource_size(path) {
            return (size.allocated_bytes, size.measurement_method);
        }
    }

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

#[cfg(target_os = "macos")]
fn macos_url_resource_size(path: &Path) -> Option<FileSize> {
    let logical_bytes = macos_url_resource_number(
        path,
        &[
            unsafe { core_foundation_sys::url::kCFURLTotalFileSizeKey },
            unsafe { core_foundation_sys::url::kCFURLFileSizeKey },
        ],
    )?;
    let allocated_bytes = macos_url_resource_number(
        path,
        &[
            unsafe { core_foundation_sys::url::kCFURLTotalFileAllocatedSizeKey },
            unsafe { core_foundation_sys::url::kCFURLFileAllocatedSizeKey },
        ],
    )
    .unwrap_or(logical_bytes);

    Some(FileSize {
        allocated_bytes,
        logical_bytes,
        measurement_method: MeasurementMethod::MacOsUrlResource,
    })
}

#[cfg(target_os = "macos")]
fn macos_url_resource_number(
    path: &Path,
    keys: &[core_foundation_sys::string::CFStringRef],
) -> Option<u64> {
    use std::ffi::c_void;
    use std::os::unix::ffi::OsStrExt;
    use std::ptr;

    use core_foundation_sys::base::{Boolean, CFRelease, CFTypeRef};
    use core_foundation_sys::error::CFErrorRef;
    use core_foundation_sys::number::{CFNumberGetValue, kCFNumberSInt64Type};
    use core_foundation_sys::url::{CFURLCreateFromFileSystemRepresentation, CFURLRef};

    unsafe extern "C" {
        fn CFURLCopyResourcePropertyForKey(
            url: CFURLRef,
            key: core_foundation_sys::string::CFStringRef,
            property_value_type_ref_ptr: *mut CFTypeRef,
            error: *mut CFErrorRef,
        ) -> Boolean;
    }

    let bytes = path.as_os_str().as_bytes();
    let url = unsafe {
        CFURLCreateFromFileSystemRepresentation(
            ptr::null(),
            bytes.as_ptr(),
            bytes.len() as isize,
            false as Boolean,
        )
    };
    if url.is_null() {
        return None;
    }

    let mut result = None;
    for key in keys {
        let mut value: CFTypeRef = ptr::null();
        let found =
            unsafe { CFURLCopyResourcePropertyForKey(url, *key, &mut value, ptr::null_mut()) };
        if found == 0 || value.is_null() {
            continue;
        }

        let mut signed_value = 0i64;
        let converted = unsafe {
            CFNumberGetValue(
                value.cast(),
                kCFNumberSInt64Type,
                (&mut signed_value as *mut i64).cast::<c_void>(),
            )
        };
        unsafe {
            CFRelease(value);
        }
        if converted && signed_value >= 0 {
            result = Some(signed_value as u64);
            break;
        }
    }

    unsafe {
        CFRelease(url.cast());
    }
    result
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

    #[cfg(target_os = "macos")]
    #[test]
    fn measurement_uses_macos_url_resource_logical_size() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("file.txt");
        std::fs::write(&path, b"hello").unwrap();

        let metadata = std::fs::metadata(&path).unwrap();
        let mut dedupe = HardLinkDedupe::new();
        let measurement = dedupe.measure_file(&path, &metadata);

        assert_eq!(
            measurement.measurement_method,
            MeasurementMethod::MacOsUrlResource
        );
        assert_eq!(measurement.logical_bytes, 5);
        assert!(measurement.allocated_bytes >= 5);
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

    #[cfg(unix)]
    #[test]
    fn hard_linked_file_counts_allocated_size_per_path_when_dedupe_disabled() {
        let dir = tempdir().unwrap();
        let first = dir.path().join("first.bin");
        let second = dir.path().join("second.bin");
        std::fs::write(&first, b"linked").unwrap();
        std::fs::hard_link(&first, &second).unwrap();

        let mut dedupe = HardLinkDedupe::new();
        let first_size =
            dedupe.measure_file_with_policy(&first, &std::fs::metadata(&first).unwrap(), false);
        let second_size =
            dedupe.measure_file_with_policy(&second, &std::fs::metadata(&second).unwrap(), false);

        assert!(first_size.allocated_bytes > 0);
        assert_eq!(second_size.allocated_bytes, first_size.allocated_bytes);
        assert!(!second_size.deduped);
    }
}
