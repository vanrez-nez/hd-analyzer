use std::collections::HashMap;
use std::path::Path;

use crate::{CategoryUsage, FileKind};

pub fn collect_categories(categories: HashMap<FileKind, u64>) -> Vec<CategoryUsage> {
    let mut categories: Vec<_> = categories
        .into_iter()
        .map(|(kind, size)| CategoryUsage { kind, size })
        .collect();
    categories.sort_by(|left, right| {
        right
            .size
            .cmp(&left.size)
            .then_with(|| left.kind.cmp(&right.kind))
    });
    categories
}

pub fn detect_file_kind(path: &Path) -> FileKind {
    let Some(extension) = normalized_extension(path) else {
        return if is_probably_executable(path) {
            FileKind::Apps
        } else {
            FileKind::Other
        };
    };

    match extension.as_str() {
        "mp4" | "mkv" | "mov" | "avi" | "m4v" | "webm" | "flv" | "mpg" | "mpeg" | "wmv" | "ts" => {
            FileKind::Video
        }
        "mp3" | "wav" | "flac" | "m4a" | "aac" | "ogg" | "opus" | "alac" | "aiff" | "wma" => {
            FileKind::Audio
        }
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "heic" | "tif" | "tiff" | "raw"
        | "svg" | "psd" => FileKind::Images,
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "tgz" | "tbz" | "iso" | "dmg" => {
            FileKind::Archives
        }
        "app" | "pkg" | "exe" | "msi" | "appimage" | "dylib" | "dll" | "so" | "deb" | "rpm" => {
            FileKind::Apps
        }
        "pdf" | "doc" | "docx" | "ppt" | "pptx" | "xls" | "xlsx" | "pages" | "numbers" | "key"
        | "txt" | "rtf" | "md" | "odt" | "epub" => FileKind::Documents,
        "rs" | "go" | "py" | "js" | "tsx" | "jsx" | "java" | "kt" | "swift" | "c" | "cc"
        | "cpp" | "h" | "hpp" | "cs" | "php" | "rb" | "lua" | "toml" | "yaml" | "yml" | "json"
        | "xml" | "sql" | "sh" | "zsh" | "bash" => FileKind::Code,
        _ => FileKind::Other,
    }
}

pub fn normalized_extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
}

fn is_probably_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = path.metadata() {
            return metadata.permissions().mode() & 0o111 != 0;
        }
    }

    #[cfg(windows)]
    {
        let executable_exts = ["exe", "bat", "cmd", "com"];
        if let Some(ext) = normalized_extension(path) {
            return executable_exts.contains(&ext.as_str());
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn classifies_common_code_extensions() {
        assert_eq!(detect_file_kind(Path::new("main.rs")), FileKind::Code);
        assert_eq!(detect_file_kind(Path::new("config.JSON")), FileKind::Code);
    }

    #[test]
    fn classifies_unknown_extensions_as_other() {
        assert_eq!(
            detect_file_kind(Path::new("payload.unknown")),
            FileKind::Other
        );
    }
}
