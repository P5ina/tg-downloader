use std::{fs, path::PathBuf};

pub struct TempFile {
    path: PathBuf,
}

impl TempFile {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if self.path.exists() {
            // if let Err(e) = fs::remove_file(&self.path) {
            //     eprintln!("Failed to remove file {:?}: {}", self.path, e);
            // }
        }
    }
}
