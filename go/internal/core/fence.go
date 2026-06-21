package core

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"
)

// Fence enforces a security boundary for file system operations.
// All write operations must stay within the boundary directory.
type Fence struct {
	boundary    string
	interactive bool
}

// NewFence creates a new Fence with the given boundary path.
func NewFence(boundary string) *Fence {
	return &Fence{
		boundary:    boundary,
		interactive: true,
	}
}

// NewFenceFromConfig creates a Fence from a GlobalConfig's workspace_root.
func NewFenceFromConfig(cfg *GlobalConfig) *Fence {
	return NewFence(cfg.WorkspaceRoot)
}

// SetInteractive controls whether the fence prompts the user for confirmation
// when operations fall outside the boundary. Set to false for testing.
func (f *Fence) SetInteractive(interactive bool) {
	f.interactive = interactive
}

// Boundary returns the fence's boundary path.
func (f *Fence) Boundary() string {
	return f.boundary
}

// IsWithinBoundary checks whether the given path is inside the boundary directory.
// Uses filepath.Abs and filepath.EvalSymlinks for accurate resolution.
// Prevents prefix-based bypasses (e.g., "ws" vs "ws_evil").
func (f *Fence) IsWithinBoundary(path string) bool {
	canonicalBoundary := resolvePath(f.boundary)

	// Try to resolve the target path.
	canonicalPath := resolvePath(path)

	// Exact match.
	if canonicalPath == canonicalBoundary {
		return true
	}

	// Use filepath.Rel to check if path is within boundary.
	// If the relative path starts with "..", the path is outside.
	rel, err := filepath.Rel(canonicalBoundary, canonicalPath)
	if err != nil {
		return false
	}

	// The path is outside if the relative path starts with ".."
	if strings.HasPrefix(rel, "..") {
		return false
	}

	return true
}

// resolvePath resolves a path to its absolute, symlink-resolved form.
// For non-existent paths, it resolves the deepest existing ancestor
// and appends the remaining segments.
func resolvePath(path string) string {
	// First try absolute path.
	absPath, err := filepath.Abs(path)
	if err != nil {
		return path
	}

	// Try to evaluate symlinks on the full path.
	resolved, err := filepath.EvalSymlinks(absPath)
	if err == nil {
		return resolved
	}

	// Path doesn't exist. Walk up to find the deepest existing ancestor,
	// resolve it, and append the remaining components.
	remaining := ""
	current := absPath
	for {
		parent := filepath.Dir(current)
		if parent == current {
			// Reached the root; just return the absolute path.
			return absPath
		}
		base := filepath.Base(current)
		if remaining == "" {
			remaining = base
		} else {
			remaining = filepath.Join(base, remaining)
		}
		current = parent

		resolved, err := filepath.EvalSymlinks(current)
		if err == nil {
			return filepath.Join(resolved, remaining)
		}
	}
}

// checkPath validates the path against the boundary and prompts if outside.
func (f *Fence) checkPath(path string, operation string) error {
	if f.IsWithinBoundary(path) {
		return nil
	}

	if f.interactive {
		fmt.Fprintf(os.Stderr,
			"SECURITY WARNING\n"+
				"Operation: %s\n"+
				"Target: %s\n"+
				"This is OUTSIDE the workspace_root: %s\n"+
				"\n"+
				"Are you sure you want to proceed? [y/N] ",
			operation, path, f.boundary,
		)

		var input string
		fmt.Fscanln(os.Stdin, &input)

		if !strings.EqualFold(strings.TrimSpace(input), "y") {
			return &ConfigError{
				Message: "operation cancelled: user declined to write outside workspace_root",
			}
		}
		return nil
	}

	return &ConfigError{
		Message: fmt.Sprintf("cannot %s outside workspace_root: %s (use --force to override)", operation, path),
	}
}

// CreateDirAll creates the directory and all parent directories after checking the boundary.
func (f *Fence) CreateDirAll(path string) error {
	if err := f.checkPath(path, "create directory"); err != nil {
		return err
	}
	return os.MkdirAll(path, 0755)
}

// Write writes content to a file after checking the boundary.
func (f *Fence) Write(path string, data []byte) error {
	if err := f.checkPath(path, "write file"); err != nil {
		return err
	}
	return os.WriteFile(path, data, 0644)
}

// RemoveDirAll removes a directory and all its contents after checking the boundary.
func (f *Fence) RemoveDirAll(path string) error {
	if err := f.checkPath(path, "remove directory"); err != nil {
		return err
	}
	return os.RemoveAll(path)
}

// Global fence management.
var (
	globalFence     *Fence
	globalFenceOnce sync.Once
)

// InitGlobalFence initializes the global fence with the given boundary.
// This should be called once during startup.
func InitGlobalFence(boundary string) {
	globalFenceOnce.Do(func() {
		globalFence = NewFence(boundary)
	})
}

// GlobalFence returns the global fence instance, or nil if not initialized.
func GlobalFence() *Fence {
	return globalFence
}

// EnsureFence returns the global fence if initialized, otherwise creates
// a new one from the config.
func EnsureFence(cfg *GlobalConfig) *Fence {
	if f := GlobalFence(); f != nil {
		return NewFence(f.Boundary())
	}
	return NewFenceFromConfig(cfg)
}
