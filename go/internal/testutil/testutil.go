// Package testutil provides helpers for testing wtp functionality.
//
// All helpers accept *testing.T and use t.Helper() + t.Fatal() for clean
// error reporting. Temporary directories are cleaned up automatically via
// t.Cleanup().
package testutil

import (
	"bytes"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"testing"

	"github.com/BurntSushi/toml"
	"github.com/eddix/wtp/internal/core"
)

// SetupTestHome creates an isolated temporary directory to be used as HOME,
// sets the HOME environment variable (and USERPROFILE on Windows), and
// restores the original value on cleanup. It also unsets XDG_CONFIG_HOME
// to prevent interference from the host environment.
//
// Returns the path to the temporary home directory.
func SetupTestHome(t *testing.T) string {
	t.Helper()

	tmpDir := t.TempDir() // auto-cleaned by testing framework

	// t.Setenv automatically restores the original value when the test ends.
	t.Setenv("HOME", tmpDir)
	if runtime.GOOS == "windows" {
		t.Setenv("USERPROFILE", tmpDir)
	}
	// Unset XDG_CONFIG_HOME to prevent the host config from interfering.
	t.Setenv("XDG_CONFIG_HOME", "")

	return tmpDir
}

// SetupTestConfig writes a GlobalConfig as TOML to ~/.wtp.toml inside the
// given home directory (the first config file location in priority order).
func SetupTestConfig(t *testing.T, home string, config core.GlobalConfig) {
	t.Helper()

	configPath := filepath.Join(home, ".wtp.toml")

	var buf bytes.Buffer
	enc := toml.NewEncoder(&buf)
	if err := enc.Encode(config); err != nil {
		t.Fatalf("testutil.SetupTestConfig: encode config: %v", err)
	}

	if err := os.WriteFile(configPath, buf.Bytes(), 0644); err != nil {
		t.Fatalf("testutil.SetupTestConfig: write %s: %v", configPath, err)
	}
}

// InitGitRepo initializes a new git repository at the given path with an
// initial commit. The directory is created if it does not exist.
//
// This creates a minimal repo with one committed file so that HEAD and
// branches are valid.
func InitGitRepo(t *testing.T, path string) {
	t.Helper()

	requireGit(t)

	if err := os.MkdirAll(path, 0755); err != nil {
		t.Fatalf("testutil.InitGitRepo: mkdir %s: %v", path, err)
	}

	cmds := [][]string{
		{"git", "init"},
		{"git", "config", "user.email", "test@wtp.dev"},
		{"git", "config", "user.name", "wtp-test"},
		// Create an initial commit so HEAD exists.
		{"git", "commit", "--allow-empty", "-m", "initial commit"},
	}

	for _, args := range cmds {
		runGit(t, path, args...)
	}
}

// InitBareGitRepo initializes a bare git repository at the given path.
// The directory is created if it does not exist.
//
// Because a bare repo has no working tree, we create a normal repo first,
// then clone it as bare, to ensure HEAD and refs/heads/main exist.
func InitBareGitRepo(t *testing.T, path string) {
	t.Helper()

	requireGit(t)

	// Create a temporary normal repo first to seed the bare one.
	tmpNormal := t.TempDir()
	InitGitRepo(t, tmpNormal)

	// Clone as bare into the target path.
	// git clone --bare <source> <target>
	parent := filepath.Dir(path)
	if err := os.MkdirAll(parent, 0755); err != nil {
		t.Fatalf("testutil.InitBareGitRepo: mkdir parent: %v", err)
	}

	cmd := exec.Command("git", "clone", "--bare", tmpNormal, path)
	out, err := cmd.CombinedOutput()
	if err != nil {
		t.Fatalf("testutil.InitBareGitRepo: git clone --bare: %v\n%s", err, out)
	}
}

// SetupTestWorkspace creates a workspace directory structure at
// <wsRoot>/<name>/.wtp/worktree.toml with an empty WorktreeToml.
func SetupTestWorkspace(t *testing.T, wsRoot, name string) string {
	t.Helper()

	wsPath := filepath.Join(wsRoot, name)
	wtpDir := filepath.Join(wsPath, core.WtpDir)

	if err := os.MkdirAll(wtpDir, 0755); err != nil {
		t.Fatalf("testutil.SetupTestWorkspace: mkdir %s: %v", wtpDir, err)
	}

	// Write empty worktree.toml
	wt := core.NewWorktreeToml()
	tomlPath := filepath.Join(wtpDir, "worktree.toml")
	if err := wt.Save(tomlPath, nil); err != nil {
		t.Fatalf("testutil.SetupTestWorkspace: save worktree.toml: %v", err)
	}

	return wsPath
}

// requireGit skips the test if git is not available.
func requireGit(t *testing.T) {
	t.Helper()

	if _, err := exec.LookPath("git"); err != nil {
		t.Skip("git not available, skipping test")
	}
}

// runGit executes a git command in the given directory, failing the test on error.
func runGit(t *testing.T, dir string, args ...string) string {
	t.Helper()

	cmd := exec.Command(args[0], args[1:]...)
	cmd.Dir = dir

	out, err := cmd.CombinedOutput()
	if err != nil {
		t.Fatalf("testutil.runGit: %s: %v\n%s", args, err, out)
	}

	return string(out)
}

// RunGitCmd executes an arbitrary git command in a directory and returns
// the combined output. This is exported for tests that need to run custom
// git commands (e.g., creating branches, making commits).
func RunGitCmd(t *testing.T, dir string, args ...string) string {
	t.Helper()
	return runGit(t, dir, args...)
}
