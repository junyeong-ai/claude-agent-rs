//! Secure file handle with TOCTOU protection.

use std::ffi::CString;
use std::io::{Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::{AsFd, AsRawFd, FromRawFd, OwnedFd};

use rustix::fs::{AtFlags, Mode, OFlags, openat, renameat, unlinkat};
use uuid::Uuid;

use super::super::SecurityError;
use super::super::path::SafePath;

pub struct SecureFileHandle {
    fd: OwnedFd,
    path: SafePath,
}

impl SecureFileHandle {
    pub fn open_read(path: SafePath) -> Result<Self, SecurityError> {
        let fd = path.open(OFlags::RDONLY)?;
        Ok(Self { fd, path })
    }

    pub fn open_write(path: SafePath) -> Result<Self, SecurityError> {
        path.create_parent_dirs()?;
        let fd = path.open(OFlags::WRONLY | OFlags::CREATE | OFlags::TRUNC)?;
        Ok(Self { fd, path })
    }

    pub fn open_append(path: SafePath) -> Result<Self, SecurityError> {
        path.create_parent_dirs()?;
        let fd = path.open(OFlags::WRONLY | OFlags::CREATE | OFlags::APPEND)?;
        Ok(Self { fd, path })
    }

    pub fn for_atomic_write(path: SafePath) -> Result<Self, SecurityError> {
        path.create_parent_dirs()?;
        let fd = path
            .open(OFlags::RDONLY)
            .or_else(|_| path.open(OFlags::WRONLY | OFlags::CREATE))?;
        Ok(Self { fd, path })
    }

    pub fn path(&self) -> &SafePath {
        &self.path
    }

    pub fn display_path(&self) -> String {
        self.path.as_path().display().to_string()
    }

    pub fn read_to_string(&self) -> Result<String, SecurityError> {
        // SAFETY: self.fd is a valid OwnedFd. We create a temporary File from the raw fd,
        // use it for reading, then forget it to prevent double-close since OwnedFd owns the fd.
        let mut file = unsafe { std::fs::File::from_raw_fd(self.fd.as_raw_fd()) };
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        std::mem::forget(file);
        Ok(content)
    }

    pub fn read_bytes(&self) -> Result<Vec<u8>, SecurityError> {
        // SAFETY: Same as read_to_string - temporary File wrapper, forgotten to prevent double-close.
        let mut file = unsafe { std::fs::File::from_raw_fd(self.fd.as_raw_fd()) };
        let mut content = Vec::new();
        file.read_to_end(&mut content)?;
        std::mem::forget(file);
        Ok(content)
    }

    pub fn write_all(&self, content: &[u8]) -> Result<(), SecurityError> {
        // SAFETY: Same as read_to_string - temporary File wrapper, forgotten to prevent double-close.
        let mut file = unsafe { std::fs::File::from_raw_fd(self.fd.as_raw_fd()) };
        file.write_all(content)?;
        file.sync_all()?;
        std::mem::forget(file);
        Ok(())
    }

    pub fn atomic_write(&self, content: &[u8]) -> Result<(), SecurityError> {
        let filename = self
            .path
            .filename()
            .ok_or_else(|| SecurityError::InvalidPath("no filename".into()))?;

        let temp_name = format!(".{}.{}.tmp", filename.to_string_lossy(), Uuid::new_v4());
        let temp_cname = CString::new(temp_name.as_bytes())
            .map_err(|_| SecurityError::InvalidPath("invalid temp name".into()))?;

        let parent_fd = self.get_parent_fd()?;

        let temp_fd = openat(
            parent_fd.as_fd(),
            &temp_cname,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o644),
        )
        .map_err(|e| SecurityError::Io(std::io::Error::from_raw_os_error(e.raw_os_error())))?;

        // SAFETY: temp_fd is a valid fd from openat. We transfer ownership to temp_std_fd
        // and forget temp_fd to prevent double-close.
        let temp_std_fd = unsafe { OwnedFd::from_raw_fd(temp_fd.as_raw_fd()) };
        std::mem::forget(temp_fd);
        // SAFETY: temp_std_fd is valid. We create a temporary File for writing,
        // then forget it since temp_std_fd manages the fd lifetime.
        let mut temp_file = unsafe { std::fs::File::from_raw_fd(temp_std_fd.as_raw_fd()) };

        let write_result = temp_file.write_all(content);
        std::mem::forget(temp_file);

        if let Err(e) = write_result {
            let _ = unlinkat(parent_fd.as_fd(), &temp_cname, AtFlags::empty());
            return Err(SecurityError::Io(e));
        }

        rustix::fs::fsync(&temp_std_fd)
            .map_err(|e| SecurityError::Io(std::io::Error::from_raw_os_error(e.raw_os_error())))?;

        let filename_cstr = CString::new(filename.as_bytes())
            .map_err(|_| SecurityError::InvalidPath("invalid filename".into()))?;

        renameat(
            parent_fd.as_fd(),
            &temp_cname,
            parent_fd.as_fd(),
            &filename_cstr,
        )
        .map_err(|e| SecurityError::Io(std::io::Error::from_raw_os_error(e.raw_os_error())))?;

        rustix::fs::fsync(&parent_fd)
            .map_err(|e| SecurityError::Io(std::io::Error::from_raw_os_error(e.raw_os_error())))?;

        Ok(())
    }

    fn get_parent_fd(&self) -> Result<OwnedFd, SecurityError> {
        let parent_components = self.path.parent_components();

        if parent_components.is_empty() {
            let fd = rustix::fs::openat(
                self.path.root_fd(),
                c".",
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|e| SecurityError::Io(std::io::Error::from_raw_os_error(e.raw_os_error())))?;
            // SAFETY: fd is valid from openat. Transfer ownership to std_fd, forget fd.
            let std_fd = unsafe { OwnedFd::from_raw_fd(fd.as_raw_fd()) };
            std::mem::forget(fd);
            return Ok(std_fd);
        }

        let mut current_fd = self.path.root_fd();
        let mut owned_fds: Vec<OwnedFd> = Vec::new();

        for component in parent_components {
            let c_name = CString::new(component.as_bytes())
                .map_err(|_| SecurityError::InvalidPath("null byte".into()))?;

            let fd = openat(
                current_fd,
                &c_name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|e| SecurityError::Io(std::io::Error::from_raw_os_error(e.raw_os_error())))?;

            // SAFETY: fd is valid from openat. Transfer ownership to std_fd, forget fd.
            let std_fd = unsafe { OwnedFd::from_raw_fd(fd.as_raw_fd()) };
            std::mem::forget(fd);
            owned_fds.push(std_fd);
            current_fd = owned_fds.last().expect("just pushed").as_fd();
        }

        owned_fds
            .pop()
            .ok_or_else(|| SecurityError::InvalidPath("no parent".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use std::sync::Arc;
    use tempfile::tempdir;

    fn create_safe_path(dir: &Path, filename: &str) -> SafePath {
        let root = std::fs::canonicalize(dir).unwrap();
        let root_fd = Arc::new(std::fs::File::open(&root).unwrap().into());
        SafePath::resolve(root_fd, root, Path::new(filename), 10).unwrap()
    }

    #[test]
    fn test_read_file() {
        let dir = tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        fs::write(root.join("test.txt"), "hello world").unwrap();

        let path = create_safe_path(&root, "test.txt");
        let handle = SecureFileHandle::open_read(path).unwrap();
        let content = handle.read_to_string().unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_write_file() {
        let dir = tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();

        let path = create_safe_path(&root, "output.txt");
        let handle = SecureFileHandle::open_write(path).unwrap();
        handle.write_all(b"test content").unwrap();

        let content = fs::read_to_string(root.join("output.txt")).unwrap();
        assert_eq!(content, "test content");
    }

    #[test]
    fn test_atomic_write() {
        let dir = tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();

        let path = create_safe_path(&root, "atomic.txt");
        let handle = SecureFileHandle::for_atomic_write(path.clone()).unwrap();
        handle.atomic_write(b"atomic content").unwrap();

        let content = fs::read_to_string(root.join("atomic.txt")).unwrap();
        assert_eq!(content, "atomic content");

        let entries: Vec<_> = fs::read_dir(&root).unwrap().collect();
        assert!(!entries.iter().any(|e| {
            e.as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains(".tmp")
        }));
    }

    #[test]
    fn test_atomic_write_preserves_original_on_new_file() {
        let dir = tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();

        let path = create_safe_path(&root, "new_atomic.txt");
        let handle = SecureFileHandle::for_atomic_write(path).unwrap();
        handle.atomic_write(b"new content").unwrap();

        let content = fs::read_to_string(root.join("new_atomic.txt")).unwrap();
        assert_eq!(content, "new content");
    }

    #[test]
    fn test_atomic_write_overwrites_existing() {
        let dir = tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        fs::write(root.join("existing.txt"), "original").unwrap();

        let path = create_safe_path(&root, "existing.txt");
        let handle = SecureFileHandle::for_atomic_write(path).unwrap();
        handle.atomic_write(b"updated").unwrap();

        let content = fs::read_to_string(root.join("existing.txt")).unwrap();
        assert_eq!(content, "updated");
    }

    #[test]
    fn test_for_atomic_write_does_not_truncate() {
        let dir = tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        fs::write(root.join("preserve.txt"), "original content").unwrap();

        let path = create_safe_path(&root, "preserve.txt");
        let _handle = SecureFileHandle::for_atomic_write(path).unwrap();

        let content = fs::read_to_string(root.join("preserve.txt")).unwrap();
        assert_eq!(content, "original content");
    }

    #[test]
    fn test_create_nested_dirs() {
        let dir = tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();

        let path = create_safe_path(&root, "a/b/c/file.txt");
        let handle = SecureFileHandle::open_write(path).unwrap();
        handle.write_all(b"nested").unwrap();

        let content = fs::read_to_string(root.join("a/b/c/file.txt")).unwrap();
        assert_eq!(content, "nested");
    }
}
