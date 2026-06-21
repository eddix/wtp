package core

import (
	"errors"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

// skipIfNoGit skips the test if git is not available.
func skipIfNoGit(t *testing.T) {
	t.Helper()
	if _, err := exec.LookPath("git"); err != nil {
		t.Skip("git not found in PATH, skipping test")
	}
}

// initTestRepo creates a temporary git repository with an initial commit.
// Returns the path to the repo and a cleanup function.
func initTestRepo(t *testing.T) string {
	t.Helper()
	dir := t.TempDir()

	runCmd(t, dir, "git", "init")
	runCmd(t, dir, "git", "config", "user.email", "test@test.com")
	runCmd(t, dir, "git", "config", "user.name", "Test")

	// Create initial commit
	testFile := filepath.Join(dir, "README.md")
	if err := os.WriteFile(testFile, []byte("# Test\n"), 0644); err != nil {
		t.Fatal(err)
	}
	runCmd(t, dir, "git", "add", ".")
	runCmd(t, dir, "git", "commit", "-m", "initial commit")

	return dir
}

// initBareTestRepo creates a temporary bare git repository.
// Returns the path to the bare repo.
func initBareTestRepo(t *testing.T) string {
	t.Helper()
	dir := t.TempDir()
	runCmd(t, dir, "git", "init", "--bare")
	return dir
}

// runCmd runs a command in the given directory and fails the test if it errors.
func runCmd(t *testing.T, dir string, name string, args ...string) string {
	t.Helper()
	cmd := exec.Command(name, args...)
	cmd.Dir = dir
	out, err := cmd.CombinedOutput()
	if err != nil {
		t.Fatalf("command %s %s failed: %v\noutput: %s", name, strings.Join(args, " "), err, string(out))
	}
	return strings.TrimSpace(string(out))
}

func TestCheckGit(t *testing.T) {
	skipIfNoGit(t)
	git := NewGitClient()
	if err := git.CheckGit(); err != nil {
		t.Fatalf("CheckGit() failed: %v", err)
	}
}

func TestGetRepoRoot_NormalRepo(t *testing.T) {
	skipIfNoGit(t)
	repoDir := initTestRepo(t)
	git := NewGitClient()

	root, err := git.GetRepoRoot(repoDir)
	if err != nil {
		t.Fatalf("GetRepoRoot() failed: %v", err)
	}

	// Resolve both to handle symlinks (e.g., /tmp -> /private/tmp on macOS)
	expected, _ := filepath.EvalSymlinks(repoDir)
	got, _ := filepath.EvalSymlinks(root)
	if got != expected {
		t.Errorf("GetRepoRoot() = %q, want %q", got, expected)
	}
}

func TestGetRepoRoot_BareRepo(t *testing.T) {
	skipIfNoGit(t)
	bareDir := initBareTestRepo(t)
	git := NewGitClient()

	root, err := git.GetRepoRoot(bareDir)
	if err != nil {
		t.Fatalf("GetRepoRoot() for bare repo failed: %v", err)
	}

	// Resolve symlinks for comparison
	expected, _ := filepath.EvalSymlinks(bareDir)
	got, _ := filepath.EvalSymlinks(root)
	if got != expected {
		t.Errorf("GetRepoRoot() bare = %q, want %q", got, expected)
	}
}

func TestGetRepoRoot_NotARepo(t *testing.T) {
	skipIfNoGit(t)
	dir := t.TempDir()
	git := NewGitClient()

	_, err := git.GetRepoRoot(dir)
	if err == nil {
		t.Fatal("GetRepoRoot() should fail for non-repo directory")
	}
	if err != ErrNotInGitRepo {
		t.Errorf("expected ErrNotInGitRepo, got %v", err)
	}
}

func TestGetRepoRoot_Subdirectory(t *testing.T) {
	skipIfNoGit(t)
	repoDir := initTestRepo(t)
	git := NewGitClient()

	// Create a subdirectory
	subDir := filepath.Join(repoDir, "sub", "dir")
	if err := os.MkdirAll(subDir, 0755); err != nil {
		t.Fatal(err)
	}

	root, err := git.GetRepoRoot(subDir)
	if err != nil {
		t.Fatalf("GetRepoRoot() from subdirectory failed: %v", err)
	}

	expected, _ := filepath.EvalSymlinks(repoDir)
	got, _ := filepath.EvalSymlinks(root)
	if got != expected {
		t.Errorf("GetRepoRoot() from subdir = %q, want %q", got, expected)
	}
}

func TestIsBareRepo(t *testing.T) {
	skipIfNoGit(t)
	git := NewGitClient()

	t.Run("bare repo", func(t *testing.T) {
		bareDir := initBareTestRepo(t)
		isBare, err := git.IsBareRepo(bareDir)
		if err != nil {
			t.Fatalf("IsBareRepo() failed: %v", err)
		}
		if !isBare {
			t.Error("IsBareRepo() = false, want true for bare repo")
		}
	})

	t.Run("normal repo", func(t *testing.T) {
		repoDir := initTestRepo(t)
		isBare, err := git.IsBareRepo(repoDir)
		if err != nil {
			t.Fatalf("IsBareRepo() failed: %v", err)
		}
		if isBare {
			t.Error("IsBareRepo() = true, want false for normal repo")
		}
	})
}

func TestIsInGitRepo(t *testing.T) {
	skipIfNoGit(t)
	git := NewGitClient()

	t.Run("in repo", func(t *testing.T) {
		repoDir := initTestRepo(t)
		if !git.IsInGitRepo(repoDir) {
			t.Error("IsInGitRepo() = false, want true")
		}
	})

	t.Run("not in repo", func(t *testing.T) {
		dir := t.TempDir()
		if git.IsInGitRepo(dir) {
			t.Error("IsInGitRepo() = true, want false")
		}
	})
}

func TestGetCurrentBranch(t *testing.T) {
	skipIfNoGit(t)
	repoDir := initTestRepo(t)
	git := NewGitClient()

	branch, err := git.GetCurrentBranch(repoDir)
	if err != nil {
		t.Fatalf("GetCurrentBranch() failed: %v", err)
	}

	// Default branch could be "main" or "master" depending on git config
	if branch == "" {
		t.Error("GetCurrentBranch() returned empty string")
	}
}

func TestGetHeadCommit(t *testing.T) {
	skipIfNoGit(t)
	repoDir := initTestRepo(t)
	git := NewGitClient()

	t.Run("short", func(t *testing.T) {
		commit, err := git.GetHeadCommit(repoDir, true)
		if err != nil {
			t.Fatalf("GetHeadCommit(short) failed: %v", err)
		}
		if len(commit) < 7 {
			t.Errorf("GetHeadCommit(short) = %q, expected at least 7 chars", commit)
		}
	})

	t.Run("full", func(t *testing.T) {
		commit, err := git.GetHeadCommit(repoDir, false)
		if err != nil {
			t.Fatalf("GetHeadCommit(full) failed: %v", err)
		}
		if len(commit) != 40 {
			t.Errorf("GetHeadCommit(full) = %q, expected 40 chars", commit)
		}
	})
}

func TestBranchExists(t *testing.T) {
	skipIfNoGit(t)
	repoDir := initTestRepo(t)
	git := NewGitClient()

	// Get the current branch name
	currentBranch, err := git.GetCurrentBranch(repoDir)
	if err != nil {
		t.Fatal(err)
	}

	t.Run("existing branch", func(t *testing.T) {
		exists, err := git.BranchExists(repoDir, currentBranch)
		if err != nil {
			t.Fatalf("BranchExists() failed: %v", err)
		}
		if !exists {
			t.Errorf("BranchExists(%q) = false, want true", currentBranch)
		}
	})

	t.Run("non-existing branch", func(t *testing.T) {
		exists, err := git.BranchExists(repoDir, "nonexistent-branch-xyz")
		if err != nil {
			t.Fatalf("BranchExists() failed: %v", err)
		}
		if exists {
			t.Error("BranchExists(nonexistent) = true, want false")
		}
	})
}

func TestGetStatus_Clean(t *testing.T) {
	skipIfNoGit(t)
	repoDir := initTestRepo(t)
	git := NewGitClient()

	status, err := git.GetStatus(repoDir)
	if err != nil {
		t.Fatalf("GetStatus() failed: %v", err)
	}

	if status.Dirty {
		t.Error("expected clean repo, got dirty")
	}
	if status.Staged != 0 || status.Unstaged != 0 || status.Untracked != 0 {
		t.Errorf("expected all zeros, got staged=%d unstaged=%d untracked=%d",
			status.Staged, status.Unstaged, status.Untracked)
	}
}

func TestGetStatus_Dirty(t *testing.T) {
	skipIfNoGit(t)
	repoDir := initTestRepo(t)
	git := NewGitClient()

	// Create an untracked file
	if err := os.WriteFile(filepath.Join(repoDir, "untracked.txt"), []byte("hello"), 0644); err != nil {
		t.Fatal(err)
	}

	// Modify tracked file (unstaged)
	if err := os.WriteFile(filepath.Join(repoDir, "README.md"), []byte("modified\n"), 0644); err != nil {
		t.Fatal(err)
	}

	// Stage a new file
	stagedFile := filepath.Join(repoDir, "staged.txt")
	if err := os.WriteFile(stagedFile, []byte("staged content"), 0644); err != nil {
		t.Fatal(err)
	}
	runCmd(t, repoDir, "git", "add", "staged.txt")

	status, err := git.GetStatus(repoDir)
	if err != nil {
		t.Fatalf("GetStatus() failed: %v", err)
	}

	if !status.Dirty {
		t.Error("expected dirty repo, got clean")
	}
	if status.Staged != 1 {
		t.Errorf("expected 1 staged, got %d", status.Staged)
	}
	if status.Unstaged != 1 {
		t.Errorf("expected 1 unstaged, got %d", status.Unstaged)
	}
	if status.Untracked != 1 {
		t.Errorf("expected 1 untracked, got %d", status.Untracked)
	}
}

func TestGetLastCommitSubject(t *testing.T) {
	skipIfNoGit(t)
	repoDir := initTestRepo(t)
	git := NewGitClient()

	subject, err := git.GetLastCommitSubject(repoDir)
	if err != nil {
		t.Fatalf("GetLastCommitSubject() failed: %v", err)
	}
	if subject != "initial commit" {
		t.Errorf("GetLastCommitSubject() = %q, want %q", subject, "initial commit")
	}
}

func TestGetLastCommitRelativeTime(t *testing.T) {
	skipIfNoGit(t)
	repoDir := initTestRepo(t)
	git := NewGitClient()

	relTime, err := git.GetLastCommitRelativeTime(repoDir)
	if err != nil {
		t.Fatalf("GetLastCommitRelativeTime() failed: %v", err)
	}
	// Should contain "ago" or "just now" or similar
	if relTime == "" {
		t.Error("GetLastCommitRelativeTime() returned empty string")
	}
}

func TestCreateWorktreeWithBranch(t *testing.T) {
	skipIfNoGit(t)
	repoDir := initTestRepo(t)
	git := NewGitClient()

	wtDir := filepath.Join(t.TempDir(), "worktree1")

	err := git.CreateWorktreeWithBranch(repoDir, wtDir, "feature-branch", "HEAD")
	if err != nil {
		t.Fatalf("CreateWorktreeWithBranch() failed: %v", err)
	}

	// Verify the worktree exists
	if _, err := os.Stat(wtDir); os.IsNotExist(err) {
		t.Error("worktree directory was not created")
	}

	// Verify the branch was created
	branch, err := git.GetCurrentBranch(wtDir)
	if err != nil {
		t.Fatalf("GetCurrentBranch() in worktree failed: %v", err)
	}
	if branch != "feature-branch" {
		t.Errorf("branch in worktree = %q, want %q", branch, "feature-branch")
	}
}

func TestAddWorktreeForBranch(t *testing.T) {
	skipIfNoGit(t)
	repoDir := initTestRepo(t)
	git := NewGitClient()

	// Create a branch first
	runCmd(t, repoDir, "git", "branch", "existing-branch")

	wtDir := filepath.Join(t.TempDir(), "worktree2")

	err := git.AddWorktreeForBranch(repoDir, wtDir, "existing-branch")
	if err != nil {
		t.Fatalf("AddWorktreeForBranch() failed: %v", err)
	}

	// Verify the worktree exists and is on the right branch
	branch, err := git.GetCurrentBranch(wtDir)
	if err != nil {
		t.Fatalf("GetCurrentBranch() in worktree failed: %v", err)
	}
	if branch != "existing-branch" {
		t.Errorf("branch in worktree = %q, want %q", branch, "existing-branch")
	}
}

func TestRemoveWorktree(t *testing.T) {
	skipIfNoGit(t)
	repoDir := initTestRepo(t)
	git := NewGitClient()

	// Create a worktree to remove
	wtDir := filepath.Join(t.TempDir(), "wt-to-remove")
	err := git.CreateWorktreeWithBranch(repoDir, wtDir, "temp-branch", "HEAD")
	if err != nil {
		t.Fatalf("setup: CreateWorktreeWithBranch() failed: %v", err)
	}

	// Remove it
	err = git.RemoveWorktree(repoDir, wtDir, false)
	if err != nil {
		t.Fatalf("RemoveWorktree() failed: %v", err)
	}

	// Verify it's gone
	if _, err := os.Stat(wtDir); !os.IsNotExist(err) {
		t.Error("worktree directory still exists after removal")
	}
}

func TestRemoveWorktree_Force(t *testing.T) {
	skipIfNoGit(t)
	repoDir := initTestRepo(t)
	git := NewGitClient()

	// Create a worktree and make it dirty
	wtDir := filepath.Join(t.TempDir(), "wt-dirty")
	err := git.CreateWorktreeWithBranch(repoDir, wtDir, "dirty-branch", "HEAD")
	if err != nil {
		t.Fatalf("setup: CreateWorktreeWithBranch() failed: %v", err)
	}

	// Make it dirty
	if err := os.WriteFile(filepath.Join(wtDir, "dirty.txt"), []byte("dirty"), 0644); err != nil {
		t.Fatal(err)
	}
	runCmd(t, wtDir, "git", "add", "dirty.txt")

	// Force remove
	err = git.RemoveWorktree(repoDir, wtDir, true)
	if err != nil {
		t.Fatalf("RemoveWorktree(force) failed: %v", err)
	}
}

func TestGetStashCount(t *testing.T) {
	skipIfNoGit(t)
	repoDir := initTestRepo(t)
	git := NewGitClient()

	t.Run("no stashes", func(t *testing.T) {
		count, err := git.GetStashCount(repoDir)
		if err != nil {
			t.Fatalf("GetStashCount() failed: %v", err)
		}
		if count != 0 {
			t.Errorf("GetStashCount() = %d, want 0", count)
		}
	})

	t.Run("with stash", func(t *testing.T) {
		// Create a file and stash it
		if err := os.WriteFile(filepath.Join(repoDir, "stash-me.txt"), []byte("stash"), 0644); err != nil {
			t.Fatal(err)
		}
		runCmd(t, repoDir, "git", "add", "stash-me.txt")
		runCmd(t, repoDir, "git", "stash")

		count, err := git.GetStashCount(repoDir)
		if err != nil {
			t.Fatalf("GetStashCount() failed: %v", err)
		}
		if count != 1 {
			t.Errorf("GetStashCount() = %d, want 1", count)
		}
	})
}

func TestCreateWorktreeWithBranch_AlreadyCheckedOut(t *testing.T) {
	skipIfNoGit(t)
	repoDir := initTestRepo(t)
	git := NewGitClient()

	// Create first worktree
	wt1 := filepath.Join(t.TempDir(), "wt1")
	err := git.CreateWorktreeWithBranch(repoDir, wt1, "my-branch", "HEAD")
	if err != nil {
		t.Fatalf("first CreateWorktreeWithBranch() failed: %v", err)
	}

	// Try to create second worktree with same branch
	wt2 := filepath.Join(t.TempDir(), "wt2")
	err = git.AddWorktreeForBranch(repoDir, wt2, "my-branch")
	if err == nil {
		t.Fatal("expected error for already checked out branch, got nil")
	}

	var branchErr *BranchAlreadyCheckedOutError
	if !errors.As(err, &branchErr) {
		t.Errorf("expected BranchAlreadyCheckedOutError, got %T: %v", err, err)
	}
}

// --- GitStatus formatting tests (no git required) ---

func TestGitStatus_FormatCompact_Clean(t *testing.T) {
	s := &GitStatus{}
	result := s.FormatCompact()
	if !strings.Contains(result, "clean") {
		t.Errorf("FormatCompact() for clean status should contain 'clean', got %q", result)
	}
}

func TestGitStatus_FormatCompact_Dirty(t *testing.T) {
	s := &GitStatus{
		Dirty:     true,
		Staged:    2,
		Unstaged:  1,
		Untracked: 3,
	}
	result := s.FormatCompact()
	if !strings.Contains(result, "2 staged") {
		t.Errorf("FormatCompact() should contain '2 staged', got %q", result)
	}
	if !strings.Contains(result, "1 unstaged") {
		t.Errorf("FormatCompact() should contain '1 unstaged', got %q", result)
	}
	if !strings.Contains(result, "3 untracked") {
		t.Errorf("FormatCompact() should contain '3 untracked', got %q", result)
	}
}

func TestGitStatus_FormatCompact_AheadBehind(t *testing.T) {
	s := &GitStatus{
		Ahead:  3,
		Behind: 1,
	}
	result := s.FormatCompact()
	if !strings.Contains(result, "+3") {
		t.Errorf("FormatCompact() should contain '+3', got %q", result)
	}
	if !strings.Contains(result, "-1") {
		t.Errorf("FormatCompact() should contain '-1', got %q", result)
	}
}

func TestGitStatus_FormatDetailStatus_Clean(t *testing.T) {
	s := &GitStatus{}
	result := s.FormatDetailStatus()
	if !strings.Contains(result, "clean") {
		t.Errorf("FormatDetailStatus() should contain 'clean', got %q", result)
	}
}

func TestGitStatus_FormatDetailStatus_Dirty(t *testing.T) {
	s := &GitStatus{
		Dirty:     true,
		Staged:    1,
		Unstaged:  2,
		Untracked: 0,
	}
	result := s.FormatDetailStatus()
	if !strings.Contains(result, "1 staged") {
		t.Errorf("FormatDetailStatus() should contain '1 staged', got %q", result)
	}
	if !strings.Contains(result, "2 unstaged") {
		t.Errorf("FormatDetailStatus() should contain '2 unstaged', got %q", result)
	}
}

func TestGitStatus_FormatDetailRemote_UpToDate(t *testing.T) {
	s := &GitStatus{}
	result := s.FormatDetailRemote()
	if !strings.Contains(result, "up to date") {
		t.Errorf("FormatDetailRemote() should contain 'up to date', got %q", result)
	}
}

func TestGitStatus_FormatDetailRemote_AheadBehind(t *testing.T) {
	s := &GitStatus{
		Ahead:  5,
		Behind: 2,
	}
	result := s.FormatDetailRemote()
	if !strings.Contains(result, "+5 ahead") {
		t.Errorf("FormatDetailRemote() should contain '+5 ahead', got %q", result)
	}
	if !strings.Contains(result, "-2 behind") {
		t.Errorf("FormatDetailRemote() should contain '-2 behind', got %q", result)
	}
}

// --- Status parsing unit test (no git required) ---

func TestParseStatusOutput(t *testing.T) {
	// Simulate parsing the output of git status --porcelain --branch.
	// We test the parsing logic directly by constructing a GitClient and
	// verifying the parsing behavior through the GitStatus fields.
	tests := []struct {
		name     string
		input    string
		expected GitStatus
	}{
		{
			name:  "clean repo",
			input: "## main...origin/main",
			expected: GitStatus{
				Dirty: false,
			},
		},
		{
			name:  "ahead and behind",
			input: "## main...origin/main [ahead 3, behind 2]",
			expected: GitStatus{
				Dirty:  false,
				Ahead:  3,
				Behind: 2,
			},
		},
		{
			name:  "ahead only",
			input: "## main...origin/main [ahead 5]",
			expected: GitStatus{
				Dirty: false,
				Ahead: 5,
			},
		},
		{
			name: "mixed changes",
			input: `## main...origin/main
M  file1.txt
 M file2.txt
?? untracked.txt
A  new-file.txt`,
			expected: GitStatus{
				Dirty:     true,
				Staged:    2, // M (index) + A
				Unstaged:  1, // M (worktree)
				Untracked: 1,
			},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			status := parseGitStatusOutput(tt.input)

			if status.Dirty != tt.expected.Dirty {
				t.Errorf("Dirty = %v, want %v", status.Dirty, tt.expected.Dirty)
			}
			if status.Ahead != tt.expected.Ahead {
				t.Errorf("Ahead = %d, want %d", status.Ahead, tt.expected.Ahead)
			}
			if status.Behind != tt.expected.Behind {
				t.Errorf("Behind = %d, want %d", status.Behind, tt.expected.Behind)
			}
			if status.Staged != tt.expected.Staged {
				t.Errorf("Staged = %d, want %d", status.Staged, tt.expected.Staged)
			}
			if status.Unstaged != tt.expected.Unstaged {
				t.Errorf("Unstaged = %d, want %d", status.Unstaged, tt.expected.Unstaged)
			}
			if status.Untracked != tt.expected.Untracked {
				t.Errorf("Untracked = %d, want %d", status.Untracked, tt.expected.Untracked)
			}
		})
	}
}
