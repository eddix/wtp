//! Fence - Security boundary for file system operations
//!
//! This module ensures all file write operations stay within the workspace_root.
//! Any attempt to write outside this boundary requires explicit user confirmation.

use crate::core::error::{Result, WtpError};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Security fence for file operations
pub struct Fence {
    /// The root directory that all operations must stay within
    boundary: PathBuf,
    /// Whether to prompt for confirmation on boundary violations
    interactive: bool,
}

impl Fence {
    /// Create a new fence with the given boundary
    pub fn new(boundary: PathBuf) -> Self {
        Self {
            boundary,
            interactive: true,
        }
    }

    /// Create a new fence from global config's workspace_root
    pub fn from_config(config: &crate::core::GlobalConfig) -> Self {
        Self::new(config.workspace_root.clone())
    }

    /// Disable interactive prompts (for testing or CI)
    pub fn non_interactive(mut self) -> Self {
        self.interactive = false;
        self
    }

    /// Check if a path is within the boundary
    pub fn is_within_boundary(&self, path: &Path) -> bool {
        let canonical_boundary = match self.boundary.canonicalize() {
            Ok(path) => path,
            Err(_) => self.boundary.clone(),
        };

        if let Ok(canonical_path) = path.canonicalize() {
            return canonical_path == canonical_boundary
                || canonical_path.starts_with(&canonical_boundary);
        }

        let candidate = if path.is_absolute() {
            path.to_path_buf()
        } else {
            canonical_boundary.join(path)
        };

        candidate == canonical_boundary || candidate.starts_with(&canonical_boundary)
    }

    /// Check path and prompt if outside boundary
    fn check_path(&self, path: &Path, operation: &str) -> Result<()> {
        if self.is_within_boundary(path) {
            return Ok(());
        }

        // Outside boundary - need confirmation
        let prompt = format!(
            "⚠️  SECURITY WARNING\n\
             Operation: {}\n\
             Target: {}\n\
             This is OUTSIDE the workspace_root: {}\n\
             \n\
             Are you sure you want to proceed? [y/N] ",
            operation,
            path.display(),
            self.boundary.display()
        );

        if self.interactive {
            eprintln!("{}", prompt);
            std::io::stderr().flush()?;

            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;

            if !input.trim().eq_ignore_ascii_case("y") {
                return Err(WtpError::config(
                    "Operation cancelled: user declined to write outside workspace_root"
                ));
            }
        } else {
            return Err(WtpError::config(format!(
                "Cannot {} outside workspace_root: {} (use --force to override)",
                operation,
                path.display()
            )));
        }

        Ok(())
    }

    /// Create directory and all parent directories
    pub fn create_dir_all(&self, path: &Path) -> Result<()> {
        self.check_path(path, "create directory")?;
        std::fs::create_dir_all(path)?;
        Ok(())
    }

    /// Write content to file
    pub fn write(&self, path: &Path, content: impl AsRef<[u8]>) -> Result<()> {
        self.check_path(path, "write file")?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Remove directory and all contents
    pub fn remove_dir_all(&self, path: &Path) -> Result<()> {
        self.check_path(path, "remove directory")?;
        std::fs::remove_dir_all(path)?;
        Ok(())
    }

    /// Remove a file
    pub fn remove_file(&self, path: &Path) -> Result<()> {
        self.check_path(path, "remove file")?;
        std::fs::remove_file(path)?;
        Ok(())
    }

    /// Get the boundary path
    pub fn boundary(&self) -> &Path {
        &self.boundary
    }
}

/// Global fence instance (lazy initialization)
use std::sync::OnceLock;
static GLOBAL_FENCE: OnceLock<Fence> = OnceLock::new();

/// Initialize the global fence
pub fn init_global_fence(boundary: PathBuf) {
    let _ = GLOBAL_FENCE.set(Fence::new(boundary));
}

/// Get the global fence
pub fn global_fence() -> Option<&'static Fence> {
    GLOBAL_FENCE.get()
}

/// Ensure fence is initialized, otherwise use default
pub fn ensure_fence(config: &crate::core::GlobalConfig) -> Fence {
    match global_fence() {
        Some(f) => Fence::new(f.boundary().to_path_buf()),
        None => Fence::from_config(config),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_within_boundary() {
        let temp = tempfile::tempdir().unwrap();
        let boundary = temp.path().to_path_buf();
        let fence = Fence::new(boundary.clone()).non_interactive();

        let inside = boundary.join("subdir/file.txt");
        assert!(fence.is_within_boundary(&inside));

        let outside = PathBuf::from("/etc/passwd");
        assert!(!fence.is_within_boundary(&outside));
    }

    #[test]
    fn test_prefix_path_bypass_prevented() {
        let temp = tempfile::tempdir().unwrap();
        let boundary = temp.path().join("ws");
        std::fs::create_dir_all(&boundary).unwrap();

        let fence = Fence::new(boundary.clone()).non_interactive();

        let outside_with_same_prefix = temp.path().join("ws_evil").join("file.txt");
        assert!(!fence.is_within_boundary(&outside_with_same_prefix));

        let inside = boundary.join("repo").join("file.txt");
        assert!(fence.is_within_boundary(&inside));
    }

    #[test]
    fn test_create_dir_all_within_boundary() {
        let temp = tempfile::tempdir().unwrap();
        let boundary = temp.path().to_path_buf();
        let fence = Fence::new(boundary.clone()).non_interactive();

        let new_dir = boundary.join("test/nested/dir");
        fence.create_dir_all(&new_dir).unwrap();
        assert!(new_dir.exists());
    }

    #[test]
    fn test_write_outside_boundary_fails() {
        let temp = tempfile::tempdir().unwrap();
        let boundary = temp.path().to_path_buf();
        let fence = Fence::new(boundary).non_interactive();

        let outside = PathBuf::from("/tmp/wtp_test_outside.txt");
        let result = fence.write(&outside, b"test");
        assert!(result.is_err());
    }
}
