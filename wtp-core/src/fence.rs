//! Fence - Security boundary for file system operations
//!
//! This module ensures all file write operations stay within the workspace_root.
//! Any attempt to write outside this boundary requires explicit user confirmation.

use crate::error::{Result, WtpError};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

/// Callback for fence boundary violations requiring user confirmation.
pub trait FenceConfirm: Send + Sync {
    /// Prompt user for confirmation. Returns true if user approves.
    fn confirm(&self, prompt: &str) -> Result<bool>;
}

/// Default implementation that reads from stdin/stderr (CLI use)
pub struct StdioConfirm;

impl FenceConfirm for StdioConfirm {
    fn confirm(&self, prompt: &str) -> Result<bool> {
        eprintln!("{}", prompt);
        std::io::stderr().flush()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        Ok(input.trim().eq_ignore_ascii_case("y"))
    }
}

/// Lexically normalize a path by resolving `.` and `..` without touching the filesystem.
pub fn lexical_normalize(path: &Path) -> PathBuf {
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

/// Validate that `child` is within `parent` boundary after canonicalization.
/// Returns the canonical child path, or an error if outside boundary.
pub fn validate_within_boundary(parent: &Path, child: &Path) -> Result<PathBuf> {
    let canonical_parent = parent
        .canonicalize()
        .map_err(|e| WtpError::config(format!("Cannot canonicalize parent: {}", e)))?;

    let canonical_child = if child.exists() {
        child
            .canonicalize()
            .map_err(|e| WtpError::config(format!("Cannot canonicalize child: {}", e)))?
    } else {
        lexical_normalize(child)
    };

    if !canonical_child.starts_with(&canonical_parent) {
        return Err(WtpError::config(format!(
            "Path traversal blocked: '{}' resolves outside '{}'",
            child.display(),
            parent.display()
        )));
    }
    Ok(canonical_child)
}

/// Security fence for file operations
pub struct Fence {
    /// The root directory that all operations must stay within
    boundary: PathBuf,
    canonical_boundary: PathBuf,
    confirm: Option<Box<dyn FenceConfirm>>,
}

impl Fence {
    /// Create a new fence with the given boundary
    pub fn new(boundary: PathBuf) -> Self {
        let canonical_boundary = boundary.canonicalize().unwrap_or_else(|_| boundary.clone());
        Self {
            boundary,
            canonical_boundary,
            confirm: None,
        }
    }

    /// Create a new fence from global config's workspace_root
    pub fn from_config(config: &crate::GlobalConfig) -> Self {
        Self::new(config.workspace_root.clone())
    }

    /// Set the confirmation callback for boundary violations
    pub fn with_confirm(mut self, confirm: Box<dyn FenceConfirm>) -> Self {
        self.confirm = Some(confirm);
        self
    }

    fn effective_canonical_boundary(&self) -> PathBuf {
        self.boundary
            .canonicalize()
            .unwrap_or_else(|_| self.canonical_boundary.clone())
    }

    fn path_within_boundary(canonical_path: &Path, canonical_boundary: &Path) -> bool {
        canonical_path == canonical_boundary || canonical_path.starts_with(canonical_boundary)
    }

    fn ancestors_resolve_within_boundary(&self, path: &Path, canonical_boundary: &Path) -> bool {
        for ancestor in path.ancestors() {
            let metadata = match std::fs::symlink_metadata(ancestor) {
                Ok(metadata) => metadata,
                Err(_) => return false,
            };

            if metadata.file_type().is_symlink() {
                let resolved = match ancestor.canonicalize() {
                    Ok(resolved) => resolved,
                    Err(_) => return false,
                };

                if !Self::path_within_boundary(&resolved, canonical_boundary) {
                    return false;
                }
            }

            if ancestor == self.boundary {
                break;
            }
        }

        true
    }

    /// Check if a path is within the boundary
    pub fn is_within_boundary(&self, path: &Path) -> bool {
        let canonical_boundary = self.effective_canonical_boundary();

        if let Ok(canonical_path) = path.canonicalize() {
            return Self::path_within_boundary(&canonical_path, &canonical_boundary)
                && self.ancestors_resolve_within_boundary(path, &canonical_boundary);
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
        Self::path_within_boundary(&normalized, &canonical_boundary)
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

        if let Some(ref confirmer) = self.confirm {
            if !confirmer.confirm(&prompt)? {
                return Err(WtpError::config(
                    "Operation cancelled: user declined to write outside workspace_root",
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

    /// Safely create directories one level at a time, refusing to follow symlinks.
    fn safe_create_dir_all(&self, path: &Path) -> Result<()> {
        let enforce_boundary = self.is_within_boundary(path);
        let canonical_boundary = if enforce_boundary {
            self.effective_canonical_boundary()
        } else {
            PathBuf::new()
        };

        // Find the deepest existing ancestor
        let mut ancestors: Vec<&Path> = Vec::new();
        let mut current = path;
        while !current.exists() {
            ancestors.push(current);
            match current.parent() {
                Some(p) => current = p,
                None => break,
            }
        }

        // Reject symlinks in any existing ancestor segment before creating anything.
        let mut existing_ancestors = Vec::new();
        for ancestor in path.ancestors() {
            if ancestor.exists() {
                existing_ancestors.push(ancestor);
            }
            if ancestor == self.boundary {
                break;
            }
        }

        for ancestor in existing_ancestors.into_iter().rev() {
            let meta = std::fs::symlink_metadata(ancestor)?;
            if meta.file_type().is_symlink() {
                return Err(WtpError::config(format!(
                    "Security: ancestor path is a symlink: {}",
                    ancestor.display()
                )));
            }

            let canonical = ancestor.canonicalize()?;
            let ancestor_ok = canonical == canonical_boundary
                || canonical.starts_with(&canonical_boundary)
                || canonical_boundary.starts_with(&canonical);
            if enforce_boundary && !ancestor_ok {
                return Err(WtpError::config(format!(
                    "Security: ancestor resolves outside boundary: {}",
                    canonical.display()
                )));
            }
        }

        // Create each level, checking for symlink after each mkdir
        for dir in ancestors.into_iter().rev() {
            std::fs::create_dir(dir)?;
            // Immediately verify no symlink race
            let meta = std::fs::symlink_metadata(dir)?;
            if meta.is_symlink() {
                let _ = std::fs::remove_dir(dir);
                return Err(WtpError::config(format!(
                    "Security: directory replaced by symlink during creation: {}",
                    dir.display()
                )));
            }
        }
        Ok(())
    }

    /// Create directory and all parent directories
    pub fn create_dir_all(&self, path: &Path) -> Result<()> {
        let within = self.check_path(path, "create directory")?;
        self.safe_create_dir_all(path)?;
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
pub fn init_global_fence(boundary: PathBuf) -> std::result::Result<(), Fence> {
    GLOBAL_FENCE.set(Fence::new(boundary))
}

/// Get the global fence
pub fn global_fence() -> Option<&'static Fence> {
    GLOBAL_FENCE.get()
}

/// Ensure fence is initialized, otherwise use default
pub fn ensure_fence(config: &crate::GlobalConfig) -> Fence {
    match global_fence() {
        Some(f) => Fence::new(f.boundary().to_path_buf()),
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
        let fence = Fence::new(boundary.clone());

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

        let fence = Fence::new(boundary.clone());

        let outside_with_same_prefix = temp.path().join("ws_evil").join("file.txt");
        assert!(!fence.is_within_boundary(&outside_with_same_prefix));

        let inside = boundary.join("repo").join("file.txt");
        assert!(fence.is_within_boundary(&inside));
    }

    #[test]
    fn test_create_dir_all_within_boundary() {
        let temp = tempfile::tempdir().unwrap();
        let boundary = temp.path().to_path_buf();
        let fence = Fence::new(boundary.clone());

        let new_dir = boundary.join("test/nested/dir");
        fence.create_dir_all(&new_dir).unwrap();
        assert!(new_dir.exists());
    }

    #[test]
    fn test_write_outside_boundary_fails() {
        let temp = tempfile::tempdir().unwrap();
        let boundary = temp.path().to_path_buf();
        let fence = Fence::new(boundary);

        let outside = PathBuf::from("/tmp/wtp_test_outside.txt");
        let result = fence.write(&outside, b"test");
        assert!(result.is_err());
    }

    #[test]
    fn test_parent_dir_traversal_blocked() {
        let temp = tempfile::tempdir().unwrap();
        let boundary = temp.path().join("ws");
        std::fs::create_dir_all(&boundary).unwrap();
        let fence = Fence::new(boundary.clone());

        // "../escaped" resolves to temp/escaped which is outside ws/
        let escaped = boundary.join("../escaped");
        assert!(!fence.is_within_boundary(&escaped));

        let result = fence.create_dir_all(&escaped);
        assert!(
            result.is_err(),
            "create_dir_all should reject '..' traversal"
        );
        assert!(
            !temp.path().join("escaped").exists(),
            "directory should not have been created"
        );
    }
}
