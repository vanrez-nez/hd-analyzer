use std::path::{Path, PathBuf};

use crate::driver::{DeleteSafetyClassification, DeleteSafetyFlag, PathDeleteSafety};

const USER_IMMUTABLE: u32 = 0x0000_0002;
const USER_APPEND_ONLY: u32 = 0x0000_0004;
const SYSTEM_IMMUTABLE: u32 = 0x0002_0000;
const SYSTEM_APPEND_ONLY: u32 = 0x0004_0000;

#[derive(Debug, Default)]
struct PathSafetyFacts {
    uid: Option<u32>,
    gid: Option<u32>,
    flags: u32,
    parent_writable: Option<bool>,
    can_delete_now: Option<bool>,
    readonly: bool,
}

pub fn classify_path_safety(path: &Path) -> PathDeleteSafety {
    classify_path_safety_from_facts(path, path_safety_facts(path))
}

fn classify_path_safety_from_facts(path: &Path, facts: PathSafetyFacts) -> PathDeleteSafety {
    let policy_path = policy_path(path);
    let mut flags = Vec::new();
    let sip_protected = is_sip_protected_path(&policy_path);
    let system_owned = is_system_owned(&facts);
    let immutable = facts.flags & (USER_IMMUTABLE | SYSTEM_IMMUTABLE) != 0;
    let append_only = facts.flags & (USER_APPEND_ONLY | SYSTEM_APPEND_ONLY) != 0;
    let safe_junk = is_safe_junk_path(&policy_path);
    let review_path = is_review_required_path(&policy_path);
    let can_delete_now = facts
        .can_delete_now
        .unwrap_or_else(|| fallback_can_delete_now(&facts));

    if system_owned {
        flags.push(DeleteSafetyFlag::SystemOwned);
    }
    if sip_protected {
        flags.push(DeleteSafetyFlag::SipProtected);
    }
    if immutable {
        flags.push(DeleteSafetyFlag::Immutable);
    }
    if append_only {
        flags.push(DeleteSafetyFlag::AppendOnly);
    }
    if facts.parent_writable == Some(false) {
        flags.push(DeleteSafetyFlag::ParentNotWritable);
    }
    if !can_delete_now {
        flags.push(DeleteSafetyFlag::NotDeletableNow);
    }
    if safe_junk {
        flags.push(DeleteSafetyFlag::SafeJunkRule);
    }

    let (classification, reason) = if sip_protected || facts.flags & SYSTEM_IMMUTABLE != 0 {
        (
            DeleteSafetyClassification::ProtectedSystem,
            "Protected system path",
        )
    } else if immutable || append_only || !can_delete_now {
        (
            DeleteSafetyClassification::NotDeletableNow,
            "Not deletable by the current app",
        )
    } else if safe_junk {
        (
            DeleteSafetyClassification::SafeJunk,
            "Matched safe junk rule",
        )
    } else if system_owned || review_path {
        (
            DeleteSafetyClassification::ReviewRequired,
            "System-owned or sensitive location",
        )
    } else {
        (DeleteSafetyClassification::UserContent, "User content")
    };

    PathDeleteSafety {
        classification,
        can_delete_now,
        flags,
        reason: reason.to_string(),
    }
}

fn fallback_can_delete_now(facts: &PathSafetyFacts) -> bool {
    if facts.readonly || facts.flags & (USER_IMMUTABLE | SYSTEM_IMMUTABLE) != 0 {
        return false;
    }

    facts.parent_writable.unwrap_or(false)
}

fn path_safety_facts(path: &Path) -> PathSafetyFacts {
    let metadata = std::fs::symlink_metadata(path).ok();
    let parent_writable = path.parent().map(parent_is_writable);

    PathSafetyFacts {
        uid: metadata.as_ref().and_then(metadata_uid),
        gid: metadata.as_ref().and_then(metadata_gid),
        flags: metadata.as_ref().map(metadata_flags).unwrap_or_default(),
        parent_writable,
        can_delete_now: platform_can_delete_now(path),
        readonly: metadata
            .as_ref()
            .is_some_and(|metadata| metadata.permissions().readonly()),
    }
}

fn policy_path(path: &Path) -> PathBuf {
    let data_volume = Path::new("/System/Volumes/Data");
    if let Ok(stripped) = path.strip_prefix(data_volume) {
        if stripped.as_os_str().is_empty() {
            return PathBuf::from("/");
        }
        return PathBuf::from("/").join(stripped);
    }

    path.to_path_buf()
}

fn is_sip_protected_path(path: &Path) -> bool {
    path == Path::new("/System")
        || path.starts_with("/System/")
        || path == Path::new("/bin")
        || path.starts_with("/bin/")
        || path == Path::new("/sbin")
        || path.starts_with("/sbin/")
        || path == Path::new("/usr")
        || (path.starts_with("/usr/") && !path.starts_with("/usr/local"))
}

fn is_review_required_path(path: &Path) -> bool {
    path == Path::new("/Applications")
        || path.starts_with("/Applications/")
        || path == Path::new("/Library")
        || path.starts_with("/Library/")
        || path.starts_with("/private/var/db")
        || path.starts_with("/var/db")
        || path.components().any(|component| {
            component.as_os_str() == "Application Support"
                || component.as_os_str() == "Keychains"
                || component.as_os_str() == "Mail"
        })
}

fn is_safe_junk_path(path: &Path) -> bool {
    let Some(home) = std::env::var_os("HOME") else {
        return false;
    };
    let home = PathBuf::from(home);
    path.starts_with(home.join("Library/Caches")) || path.starts_with(home.join("Library/Logs"))
}

fn is_system_owned(facts: &PathSafetyFacts) -> bool {
    facts.uid == Some(0) || facts.gid == Some(0) || facts.uid.is_some_and(|uid| uid < 500)
}

#[cfg(unix)]
fn parent_is_writable(path: &Path) -> bool {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return false;
    };
    mode_allows_write_execute(&metadata)
}

#[cfg(not(unix))]
fn parent_is_writable(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|metadata| !metadata.permissions().readonly())
        .unwrap_or(false)
}

#[cfg(unix)]
fn mode_allows_write_execute(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;

    let mode = metadata.mode();
    let uid = current_uid();
    let gid = current_gid();

    if uid == 0 {
        return true;
    }
    if metadata.uid() == uid {
        return mode & 0o300 == 0o300;
    }
    if metadata.gid() == gid {
        return mode & 0o030 == 0o030;
    }
    mode & 0o003 == 0o003
}

#[cfg(unix)]
fn current_uid() -> u32 {
    unsafe extern "C" {
        fn getuid() -> u32;
    }

    unsafe { getuid() }
}

#[cfg(unix)]
fn current_gid() -> u32 {
    unsafe extern "C" {
        fn getgid() -> u32;
    }

    unsafe { getgid() }
}

#[cfg(unix)]
fn metadata_uid(metadata: &std::fs::Metadata) -> Option<u32> {
    use std::os::unix::fs::MetadataExt;

    Some(metadata.uid())
}

#[cfg(not(unix))]
fn metadata_uid(_metadata: &std::fs::Metadata) -> Option<u32> {
    None
}

#[cfg(unix)]
fn metadata_gid(metadata: &std::fs::Metadata) -> Option<u32> {
    use std::os::unix::fs::MetadataExt;

    Some(metadata.gid())
}

#[cfg(not(unix))]
fn metadata_gid(_metadata: &std::fs::Metadata) -> Option<u32> {
    None
}

#[cfg(target_os = "macos")]
fn metadata_flags(metadata: &std::fs::Metadata) -> u32 {
    use std::os::macos::fs::MetadataExt;

    metadata.st_flags()
}

#[cfg(not(target_os = "macos"))]
fn metadata_flags(_metadata: &std::fs::Metadata) -> u32 {
    0
}

#[cfg(target_os = "macos")]
fn platform_can_delete_now(path: &Path) -> Option<bool> {
    use objc2_foundation::{NSFileManager, NSString};

    let path = NSString::from_str(&path.display().to_string());
    Some(NSFileManager::defaultManager().isDeletableFileAtPath(&path))
}

#[cfg(not(target_os = "macos"))]
fn platform_can_delete_now(_path: &Path) -> Option<bool> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sip_prefix_is_protected_system() {
        let safety = classify_path_safety_from_facts(
            Path::new("/System/Library"),
            PathSafetyFacts {
                can_delete_now: Some(true),
                parent_writable: Some(true),
                ..PathSafetyFacts::default()
            },
        );

        assert_eq!(
            safety.classification,
            DeleteSafetyClassification::ProtectedSystem
        );
        assert!(safety.flags.contains(&DeleteSafetyFlag::SipProtected));
    }

    #[test]
    fn data_volume_user_path_is_not_sip_protected() {
        let safety = classify_path_safety_from_facts(
            Path::new("/System/Volumes/Data/Users/user/Documents"),
            PathSafetyFacts {
                can_delete_now: Some(true),
                parent_writable: Some(true),
                uid: Some(501),
                gid: Some(20),
                ..PathSafetyFacts::default()
            },
        );

        assert_eq!(
            safety.classification,
            DeleteSafetyClassification::UserContent
        );
    }

    #[test]
    fn immutable_path_is_not_deletable_now() {
        let safety = classify_path_safety_from_facts(
            Path::new("/Users/user/file"),
            PathSafetyFacts {
                can_delete_now: Some(true),
                parent_writable: Some(true),
                flags: USER_IMMUTABLE,
                uid: Some(501),
                gid: Some(20),
                ..PathSafetyFacts::default()
            },
        );

        assert_eq!(
            safety.classification,
            DeleteSafetyClassification::NotDeletableNow
        );
        assert!(safety.flags.contains(&DeleteSafetyFlag::Immutable));
    }

    #[test]
    fn user_cache_path_is_safe_junk() {
        let home = std::env::var("HOME").unwrap();
        let safety = classify_path_safety_from_facts(
            &Path::new(&home).join("Library/Caches/example"),
            PathSafetyFacts {
                can_delete_now: Some(true),
                parent_writable: Some(true),
                uid: Some(501),
                gid: Some(20),
                ..PathSafetyFacts::default()
            },
        );

        assert_eq!(safety.classification, DeleteSafetyClassification::SafeJunk);
        assert!(safety.flags.contains(&DeleteSafetyFlag::SafeJunkRule));
    }

    #[test]
    fn system_owned_path_requires_review() {
        let safety = classify_path_safety_from_facts(
            Path::new("/opt/vendor"),
            PathSafetyFacts {
                can_delete_now: Some(true),
                parent_writable: Some(true),
                uid: Some(0),
                gid: Some(0),
                ..PathSafetyFacts::default()
            },
        );

        assert_eq!(
            safety.classification,
            DeleteSafetyClassification::ReviewRequired
        );
        assert!(safety.flags.contains(&DeleteSafetyFlag::SystemOwned));
    }
}
