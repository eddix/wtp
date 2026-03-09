//! Fence - Security boundary for file system operations
//!
//! This module ensures all file write operations stay within the workspace_root.
//! Any attempt to write outside this boundary requires explicit user confirmation.

use crate::core::error::{Result, WtpError};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

/// Lexically normalize a path by resolving `.` and `..` without touching the filesystem.
fn lexical_normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            c => out.push(c),
        }
    }
    out
}

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

    /// Disable interactive prompts (for testing)
    #[cfg(test)]
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

        // Path doesn't exist — lexically normalize to catch ".." traversal
        let candidate = if path.is_absolute() {
            if let Ok(rel_path) = path.strip_prefix(&self.boundary) {
                canonical_boundary.join(rel_path)
            } else {
                path.to_path_buf()
            }
        } else {
            canonical_boundary.join(path)
        };

        let normalized = lexical_normalize(&candidate);
        normalized == canonical_boundary || normalized.starts_with(&canonical_boundary)
    }

    /// Check path and prompt if outside boundary.
    /// Returns `true` if the path is within the boundary, `false` if the user
    /// approved an out-of-boundary override. Errors if denied.
    fn check_path(&self, path: &Path, operation: &str) -> Result<bool> {
        if self.is_within_boundary(path) {
            return Ok(true);
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

        Ok(false)
    }

    /// Create directory and all parent directories
    pub fn create_dir_all(&self, path: &Path) -> Result<()> {
        let within = self.check_path(path, "create directory")?;
        std::fs::create_dir_all(path)?;
        // Re-verify after creation to catch symlink races (only for in-boundary paths)
        if within {
            self.verify_canonical(path, "create directory")?;
        }
        Ok(())
    }

    /// Write content to file
    pub fn write(&self, path: &Path, content: impl AsRef<[u8]>) -> Result<()> {
        let within = self.check_path(path, "write file")?;
        // Re-verify parent exists and is within boundary (only for in-boundary paths)
        if within {
            if let Some(parent) = path.parent() {
                if parent.exists() {
                    self.verify_canonical(parent, "write file")?;
                }
            }
        }
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Remove directory and all contents
    pub fn remove_dir_all(&self, path: &Path) -> Result<()> {
        let within = self.check_path(path, "remove directory")?;
        // Check for symlinks at the top level to prevent escaping
        if path.exists() {
            let metadata = std::fs::symlink_metadata(path)?;
            if metadata.is_symlink() {
                return Err(WtpError::config(format!(
                    "Refusing to recursively remove a symlink: {}",
                    path.display()
                )));
            }
            if within {
                self.verify_canonical(path, "remove directory")?;
            }
        }
        std::fs::remove_dir_all(path)?;
        Ok(())
    }

    /// Re-verify that a path's canonical form is within boundary.
    /// Called after I/O to mitigate TOCTOU races for in-boundary paths.
    fn verify_canonical(&self, path: &Path, operation: &str) -> Result<()> {
        if let Ok(canonical) = path.canonicalize() {
            if !self.is_within_boundary(&canonical) {
                return Err(WtpError::config(format!(
                    "Security: path resolved outside boundary during {}: {}",
                    operation,
                    canonical.display()
                )));
            }
        }
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
        Some(f) => Fence {
            boundary: f.boundary().to_path_buf(),
            interactive: f.interactive,
        },
        None => Fence::from_config(config),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_parent_dir_traversal_blocked() {
        let temp = tempfile::tempdir().unwrap();
        let boundary = temp.path().join("ws");
        std::fs::create_dir_all(&boundary).unwrap();
        let fence = Fence::new(boundary.clone()).non_interactive();

        // "../escaped" resolves to temp/escaped which is outside ws/
        let escaped = boundary.join("../escaped");
        assert!(!fence.is_within_boundary(&escaped));

        let result = fence.create_dir_all(&escaped);
        assert!(result.is_err(), "create_dir_all should reject '..' traversal");
        assert!(!temp.path().join("escaped").exists(), "directory should not have been created");
    }
}
