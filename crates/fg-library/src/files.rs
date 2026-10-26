//! The seam between the guide library and where its files live: the app's folder on the desktop,
//! the browser's storage on the web. Paths are relative to the root, separated by `/`.

use std::io;
use std::path::PathBuf;

pub trait Files {
    /// The content of a file; a missing one is `ErrorKind::NotFound`.
    ///
    /// # Errors
    ///
    /// The file system's error: `NotFound` for a missing file or folder.
    fn read(&self, path: &str) -> io::Result<Vec<u8>>;
    /// Writes a file, and the folders above it when they do not exist yet.
    ///
    /// # Errors
    ///
    /// The file system's error: `NotFound` for a missing file or folder.
    fn write(&self, path: &str, data: &[u8]) -> io::Result<()>;
    /// The names (files and folders, once each) directly in a folder; none when it does not exist.
    fn list(&self, dir: &str) -> Vec<String>;
    /// Removes a folder and everything in it; a missing one is `ErrorKind::NotFound`.
    ///
    /// # Errors
    ///
    /// The file system's error: `NotFound` for a missing file or folder.
    fn remove_dir(&self, dir: &str) -> io::Result<()>;
}

/// The files of a folder of the disk.
#[derive(Debug, Clone)]
pub struct DiskFiles {
    root: PathBuf,
}

impl DiskFiles {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl Files for DiskFiles {
    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        std::fs::read(self.root.join(path))
    }

    fn write(&self, path: &str, data: &[u8]) -> io::Result<()> {
        let file = self.root.join(path);
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(file, data)
    }

    fn list(&self, dir: &str) -> Vec<String> {
        std::fs::read_dir(self.root.join(dir))
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect()
    }

    fn remove_dir(&self, dir: &str) -> io::Result<()> {
        std::fs::remove_dir_all(self.root.join(dir))
    }
}
