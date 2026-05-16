use std::path::{Component, Path};

use humansize::{DECIMAL, format_size};

pub fn format_bytes(size: u64) -> String {
    format_size(size, DECIMAL)
}

pub fn format_ratio(size: u64, total: u64) -> String {
    if total == 0 {
        return "0.0%".to_string();
    }

    let ratio = size as f64 / total as f64 * 100.0;
    format!("{ratio:.1}%")
}

pub fn compact_path(path: &Path, root: &Path) -> String {
    if path == root {
        return root.display().to_string();
    }

    match path.strip_prefix(root) {
        Ok(stripped) => {
            let mut components = Vec::new();
            for component in stripped.components() {
                match component {
                    Component::Normal(value) => {
                        components.push(value.to_string_lossy().into_owned())
                    }
                    Component::RootDir => components.push(std::path::MAIN_SEPARATOR.to_string()),
                    _ => {}
                }
            }

            if components.is_empty() {
                root.display().to_string()
            } else {
                let prefix = root.display().to_string();
                format!(
                    "{prefix}{}{}",
                    std::path::MAIN_SEPARATOR,
                    components.join(std::path::MAIN_SEPARATOR_STR)
                )
            }
        }
        Err(_) => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_zero_total_ratio() {
        assert_eq!(format_ratio(100, 0), "0.0%");
    }

    #[test]
    fn compacts_child_path_under_root() {
        let root = Path::new("/tmp/root");
        let child = Path::new("/tmp/root/a/b");
        assert_eq!(compact_path(child, root), "/tmp/root/a/b");
    }
}
