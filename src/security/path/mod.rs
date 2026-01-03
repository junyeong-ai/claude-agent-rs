//! TOCTOU-safe path handling with symlink protection.

mod resolver;

pub use resolver::SafePath;

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

pub const DEFAULT_MAX_SYMLINK_DEPTH: u8 = 10;

pub fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();

    for component in path.components() {
        match component {
            Component::ParentDir => {
                if !components.is_empty()
                    && !matches!(
                        components.last(),
                        Some(Component::RootDir) | Some(Component::Prefix(_))
                    )
                {
                    components.pop();
                }
            }
            Component::CurDir => {}
            c => components.push(c),
        }
    }

    if components.is_empty() {
        PathBuf::from(".")
    } else {
        components.iter().collect()
    }
}

pub fn extract_relative_components(path: &Path) -> Vec<OsString> {
    path.components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_os_string()),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path() {
        assert_eq!(
            normalize_path(Path::new("/a/b/../c")),
            PathBuf::from("/a/c")
        );
        assert_eq!(normalize_path(Path::new("/a/./b")), PathBuf::from("/a/b"));
        assert_eq!(
            normalize_path(Path::new("/a/b/../../c")),
            PathBuf::from("/c")
        );
    }

    #[test]
    fn test_extract_relative_components() {
        let components = extract_relative_components(Path::new("/a/b/c"));
        assert_eq!(components.len(), 3);
        assert_eq!(components[0], "a");
        assert_eq!(components[1], "b");
        assert_eq!(components[2], "c");
    }
}
