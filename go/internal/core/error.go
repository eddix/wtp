package core

import (
	"errors"
	"fmt"
)

// Sentinel errors.
var (
	// ErrNotInGitRepo indicates the current directory is not inside a git repository.
	ErrNotInGitRepo = errors.New("not in a git repository")
)

// ConfigError represents a configuration-related error.
type ConfigError struct {
	Message string
}

func (e *ConfigError) Error() string {
	return fmt.Sprintf("configuration error: %s", e.Message)
}

// NewConfigError creates a new ConfigError.
func NewConfigError(msg string) *ConfigError {
	return &ConfigError{Message: msg}
}

// GitError represents a git operation error.
type GitError struct {
	Message string
}

func (e *GitError) Error() string {
	return fmt.Sprintf("git error: %s", e.Message)
}

// NewGitError creates a new GitError.
func NewGitError(msg string) *GitError {
	return &GitError{Message: msg}
}

// WorkspaceAlreadyExistsError indicates a workspace with the given name already exists.
type WorkspaceAlreadyExistsError struct {
	Name string
	Path string
}

func (e *WorkspaceAlreadyExistsError) Error() string {
	return fmt.Sprintf("workspace already exists: %s at %s", e.Name, e.Path)
}

// BranchAlreadyCheckedOutError indicates a branch is already checked out in another worktree.
type BranchAlreadyCheckedOutError struct {
	Branch       string
	WorktreePath string
}

func (e *BranchAlreadyCheckedOutError) Error() string {
	return fmt.Sprintf("branch '%s' is already checked out in another worktree: %s", e.Branch, e.WorktreePath)
}

// WorktreeAlreadyExistsError indicates a worktree already exists at the given path.
type WorktreeAlreadyExistsError struct {
	Path string
}

func (e *WorktreeAlreadyExistsError) Error() string {
	return fmt.Sprintf("worktree already exists: %s", e.Path)
}
