//! TOCTOU-safe path handling with symlink protection.

mod resolver;

pub use resolver::SafePath;

use std::path::{Component, Path, PathBuf};

pub(crate) const DEFAULT_MAX_SYMLINK_DEPTH: u8 = 10;

pub(crate) fn normalize_path(path: &Path) -> PathBuf {
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
}
