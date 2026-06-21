package core

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"

	"github.com/fatih/color"
)

// GitOperations defines the interface for all git operations.
// Using an interface allows mock implementations for testing.
type GitOperations interface {
	// CheckGit verifies that git is available in PATH.
	CheckGit() error

	// GetRepoRoot returns the root directory of a git repository.
	// Supports both normal and bare repositories.
	// For normal repos: returns the work tree root (--show-toplevel).
	// For bare repos: returns the git directory itself.
	GetRepoRoot(cwd string) (string, error)

	// IsBareRepo checks if the path is a bare git repository.
	IsBareRepo(path string) (bool, error)

	// IsInGitRepo checks if a directory is inside a git repository.
	IsInGitRepo(cwd string) bool

	// GetCurrentBranch returns the current branch name.
	GetCurrentBranch(repoPath string) (string, error)

	// GetHeadCommit returns the HEAD commit hash.
	// If short is true, returns the abbreviated hash.
	GetHeadCommit(repoPath string, short bool) (string, error)

	// BranchExists checks if a branch exists in the repository.
	BranchExists(repoPath, branch string) (bool, error)

	// CreateWorktreeWithBranch creates a new worktree with a new branch.
	CreateWorktreeWithBranch(repoPath, wtPath, branch, base string) error

	// AddWorktreeForBranch adds a worktree for an existing branch.
	AddWorktreeForBranch(repoPath, wtPath, branch string) error

	// GetStatus returns the detailed status of a repository.
	GetStatus(repoPath string) (*GitStatus, error)

	// GetLastCommitSubject returns the subject line of the last commit.
	GetLastCommitSubject(repoPath string) (string, error)

	// GetLastCommitRelativeTime returns the relative time of the last commit
	// (e.g., "2 hours ago").
	GetLastCommitRelativeTime(repoPath string) (string, error)

	// RemoveWorktree removes a worktree from a repository.
	RemoveWorktree(repoPath, wtPath string, force bool) error

	// GetStashCount returns the number of stash entries.
	GetStashCount(repoPath string) (int, error)
}

// GitClient implements GitOperations by executing git CLI commands.
type GitClient struct{}

// NewGitClient creates a new GitClient.
func NewGitClient() *GitClient {
	return &GitClient{}
}

// runGit executes a git command in the given directory and returns trimmed stdout.
// Returns an error if the command fails, with stderr included in the error message.
func (g *GitClient) runGit(dir string, args ...string) (string, error) {
	cmd := exec.Command("git", args...)
	if dir != "" {
		cmd.Dir = dir
	}
	out, err := cmd.Output()
	if err != nil {
		if exitErr, ok := err.(*exec.ExitError); ok {
			return "", fmt.Errorf("git %s: %s", strings.Join(args, " "), strings.TrimSpace(string(exitErr.Stderr)))
		}
		return "", fmt.Errorf("git %s: %w", strings.Join(args, " "), err)
	}
	return strings.TrimSpace(string(out)), nil
}

// runGitRaw executes a git command and returns stdout, stderr, and the exit error (if any).
// Unlike runGit, this does not wrap the error — callers can inspect stderr directly.
func (g *GitClient) runGitRaw(dir string, args ...string) (stdout, stderr string, err error) {
	cmd := exec.Command("git", args...)
	if dir != "" {
		cmd.Dir = dir
	}
	var stdoutBuf, stderrBuf strings.Builder
	cmd.Stdout = &stdoutBuf
	cmd.Stderr = &stderrBuf
	err = cmd.Run()
	return strings.TrimSpace(stdoutBuf.String()), strings.TrimSpace(stderrBuf.String()), err
}

func (g *GitClient) CheckGit() error {
	_, err := g.runGit("", "--version")
	if err != nil {
		return NewGitError("Git is not installed or not in PATH")
	}
	return nil
}

func (g *GitClient) GetRepoRoot(cwd string) (string, error) {
	// Try --show-toplevel first (works for normal repos)
	toplevel, err := g.runGit(cwd, "rev-parse", "--show-toplevel")
	if err == nil && toplevel != "" {
		return toplevel, nil
	}

	// Fall back to bare repo detection
	isBareStr, err := g.runGit(cwd, "rev-parse", "--is-bare-repository")
	if err != nil {
		return "", ErrNotInGitRepo
	}

	if isBareStr == "true" {
		gitDir, err := g.runGit(cwd, "rev-parse", "--git-dir")
		if err != nil {
			return "", ErrNotInGitRepo
		}

		if filepath.IsAbs(gitDir) {
			return gitDir, nil
		}

		// Resolve relative path against cwd
		if cwd != "" {
			absPath, err := filepath.Abs(filepath.Join(cwd, gitDir))
			if err != nil {
				return "", ErrNotInGitRepo
			}
			// Evaluate symlinks for canonical path
			resolved, err := filepath.EvalSymlinks(absPath)
			if err != nil {
				return absPath, nil
			}
			return resolved, nil
		}
	}

	return "", ErrNotInGitRepo
}

func (g *GitClient) IsBareRepo(path string) (bool, error) {
	out, err := g.runGit(path, "rev-parse", "--is-bare-repository")
	if err != nil {
		return false, err
	}
	return out == "true", nil
}

func (g *GitClient) IsInGitRepo(cwd string) bool {
	_, err := g.GetRepoRoot(cwd)
	return err == nil
}

func (g *GitClient) GetCurrentBranch(repoPath string) (string, error) {
	branch, err := g.runGit(repoPath, "rev-parse", "--abbrev-ref", "HEAD")
	if err != nil {
		return "", NewGitError(fmt.Sprintf("failed to get current branch: %v", err))
	}
	return branch, nil
}

func (g *GitClient) GetHeadCommit(repoPath string, short bool) (string, error) {
	args := []string{"rev-parse"}
	if short {
		args = append(args, "--short")
	}
	args = append(args, "HEAD")

	commit, err := g.runGit(repoPath, args...)
	if err != nil {
		return "", NewGitError(fmt.Sprintf("failed to get HEAD commit: %v", err))
	}
	return commit, nil
}

func (g *GitClient) BranchExists(repoPath, branch string) (bool, error) {
	_, err := g.runGit(repoPath, "show-ref", "--verify", "refs/heads/"+branch)
	if err != nil {
		// show-ref exits with 1 if ref not found — that's not an error for us
		return false, nil
	}
	return true, nil
}

func (g *GitClient) CreateWorktreeWithBranch(repoPath, wtPath, branch, base string) error {
	// Ensure parent directory exists
	if err := os.MkdirAll(filepath.Dir(wtPath), 0755); err != nil {
		return fmt.Errorf("create parent directory: %w", err)
	}

	_, stderr, err := g.runGitRaw(repoPath, "worktree", "add", "-b", branch, wtPath, base)
	if err != nil {
		if strings.Contains(stderr, "already checked out") {
			return &BranchAlreadyCheckedOutError{
				Branch:       branch,
				WorktreePath: wtPath,
			}
		}
		if strings.Contains(stderr, "already exists") {
			return &WorktreeAlreadyExistsError{Path: wtPath}
		}
		return NewGitError(fmt.Sprintf("failed to create worktree: %s", stderr))
	}
	return nil
}

func (g *GitClient) AddWorktreeForBranch(repoPath, wtPath, branch string) error {
	// Ensure parent directory exists
	if err := os.MkdirAll(filepath.Dir(wtPath), 0755); err != nil {
		return fmt.Errorf("create parent directory: %w", err)
	}

	_, stderr, err := g.runGitRaw(repoPath, "worktree", "add", wtPath, branch)
	if err != nil {
		if strings.Contains(stderr, "already checked out") {
			return &BranchAlreadyCheckedOutError{
				Branch:       branch,
				WorktreePath: wtPath,
			}
		}
		if strings.Contains(stderr, "already exists") {
			return &WorktreeAlreadyExistsError{Path: wtPath}
		}
		return NewGitError(fmt.Sprintf("failed to add worktree: %s", stderr))
	}
	return nil
}

func (g *GitClient) GetStatus(repoPath string) (*GitStatus, error) {
	out, err := g.runGit(repoPath, "status", "--porcelain", "--branch")
	if err != nil {
		return nil, NewGitError(fmt.Sprintf("failed to get status: %v", err))
	}

	status := parseGitStatusOutput(out)
	return status, nil
}

// parseGitStatusOutput parses the output of "git status --porcelain --branch"
// into a GitStatus struct. Exported for testing.
func parseGitStatusOutput(output string) *GitStatus {
	status := &GitStatus{}

	for _, line := range strings.Split(output, "\n") {
		if line == "" {
			continue
		}
		if strings.HasPrefix(line, "## ") {
			// Parse branch info for ahead/behind
			if idx := strings.Index(line, "[ahead "); idx >= 0 {
				rest := line[idx+7:]
				if end := strings.IndexAny(rest, ",]"); end >= 0 {
					if n, err := strconv.Atoi(strings.TrimSpace(rest[:end])); err == nil {
						status.Ahead = n
					}
				}
			}
			if idx := strings.Index(line, "behind "); idx >= 0 {
				rest := line[idx+7:]
				if end := strings.IndexByte(rest, ']'); end >= 0 {
					if n, err := strconv.Atoi(strings.TrimSpace(rest[:end])); err == nil {
						status.Behind = n
					}
				}
			}
		} else if len(line) >= 2 {
			x := line[0]
			y := line[1]

			if x == '?' && y == '?' {
				status.Untracked++
			} else {
				if x != ' ' && x != '?' {
					status.Staged++
				}
				if y != ' ' && y != '?' {
					status.Unstaged++
				}
			}
		}
	}

	status.Dirty = status.Staged > 0 || status.Unstaged > 0 || status.Untracked > 0

	return status
}

func (g *GitClient) GetLastCommitSubject(repoPath string) (string, error) {
	subject, err := g.runGit(repoPath, "log", "-1", "--format=%s")
	if err != nil {
		return "", NewGitError(fmt.Sprintf("failed to get last commit subject: %v", err))
	}
	return subject, nil
}

func (g *GitClient) GetLastCommitRelativeTime(repoPath string) (string, error) {
	relTime, err := g.runGit(repoPath, "log", "-1", "--format=%cr")
	if err != nil {
		return "", NewGitError(fmt.Sprintf("failed to get last commit time: %v", err))
	}
	return relTime, nil
}

func (g *GitClient) RemoveWorktree(repoPath, wtPath string, force bool) error {
	args := []string{"worktree", "remove"}
	if force {
		args = append(args, "--force")
	}
	args = append(args, wtPath)

	_, stderr, err := g.runGitRaw(repoPath, args...)
	if err != nil {
		return NewGitError(fmt.Sprintf("failed to remove worktree: %s", stderr))
	}
	return nil
}

func (g *GitClient) GetStashCount(repoPath string) (int, error) {
	out, err := g.runGit(repoPath, "stash", "list")
	if err != nil {
		return 0, NewGitError(fmt.Sprintf("failed to get stash list: %v", err))
	}
	if out == "" {
		return 0, nil
	}
	count := 0
	for _, line := range strings.Split(out, "\n") {
		if strings.TrimSpace(line) != "" {
			count++
		}
	}
	return count, nil
}

// GitStatus holds the status information for a git repository.
type GitStatus struct {
	// Dirty indicates if there are any uncommitted changes.
	Dirty bool
	// Ahead is the number of commits ahead of the remote.
	Ahead int
	// Behind is the number of commits behind the remote.
	Behind int
	// Staged is the number of staged files.
	Staged int
	// Unstaged is the number of modified but unstaged files.
	Unstaged int
	// Untracked is the number of untracked files.
	Untracked int
}

// FormatCompact returns a compact colored status string.
func (s *GitStatus) FormatCompact() string {
	if !s.Dirty && s.Ahead == 0 && s.Behind == 0 {
		return color.GreenString("\u2713 clean")
	}

	var parts []string

	if s.Dirty {
		var detail []string
		if s.Staged > 0 {
			detail = append(detail, fmt.Sprintf("%d staged", s.Staged))
		}
		if s.Unstaged > 0 {
			detail = append(detail, fmt.Sprintf("%d unstaged", s.Unstaged))
		}
		if s.Untracked > 0 {
			detail = append(detail, fmt.Sprintf("%d untracked", s.Untracked))
		}
		statusStr := fmt.Sprintf("* %s", strings.Join(detail, ", "))
		parts = append(parts, color.YellowString("%s", statusStr))
	}

	if s.Ahead > 0 || s.Behind > 0 {
		var remoteParts []string
		if s.Ahead > 0 {
			remoteParts = append(remoteParts, color.GreenString("+%d", s.Ahead))
		}
		if s.Behind > 0 {
			remoteParts = append(remoteParts, color.RedString("-%d", s.Behind))
		}
		parts = append(parts, fmt.Sprintf("(%s)", strings.Join(remoteParts, " ")))
	}

	return strings.Join(parts, "  ")
}

// FormatDetailStatus returns a detailed status string for the --long view.
func (s *GitStatus) FormatDetailStatus() string {
	if !s.Dirty {
		return color.GreenString("\u2713 clean")
	}

	var detail []string
	if s.Staged > 0 {
		detail = append(detail, fmt.Sprintf("%d staged", s.Staged))
	}
	if s.Unstaged > 0 {
		detail = append(detail, fmt.Sprintf("%d unstaged", s.Unstaged))
	}
	if s.Untracked > 0 {
		detail = append(detail, fmt.Sprintf("%d untracked", s.Untracked))
	}
	return color.YellowString("%s", strings.Join(detail, ", "))
}

// FormatDetailRemote returns a detailed remote tracking string for the --long view.
func (s *GitStatus) FormatDetailRemote() string {
	if s.Ahead == 0 && s.Behind == 0 {
		return color.GreenString("up to date")
	}

	var parts []string
	if s.Ahead > 0 {
		parts = append(parts, color.GreenString("+%d ahead", s.Ahead))
	}
	if s.Behind > 0 {
		parts = append(parts, color.RedString("-%d behind", s.Behind))
	}
	return strings.Join(parts, ", ")
}

// Compile-time check that GitClient implements GitOperations.
var _ GitOperations = (*GitClient)(nil)
