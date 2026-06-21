package testutil

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"github.com/eddix/wtp/internal/core"
)

func gitAvailable() bool {
	_, err := exec.LookPath("git")
	return err == nil
}

func TestSetupTestHome(t *testing.T) {
	home := SetupTestHome(t)

	if home == "" {
		t.Fatal("SetupTestHome returned empty string")
	}

	info, err := os.Stat(home)
	if err != nil {
		t.Fatalf("home dir does not exist: %v", err)
	}
	if !info.IsDir() {
		t.Fatal("home is not a directory")
	}

	// HOME should be set to the temp directory
	if got := os.Getenv("HOME"); got != home {
		t.Errorf("HOME = %q, want %q", got, home)
	}

	// XDG_CONFIG_HOME should be unset
	if got := os.Getenv("XDG_CONFIG_HOME"); got != "" {
		t.Errorf("XDG_CONFIG_HOME = %q, want empty", got)
	}
}

func TestSetupTestConfig(t *testing.T) {
	home := SetupTestHome(t)

	cfg := core.GlobalConfig{
		WorkspaceRoot: filepath.Join(home, ".wtp", "workspaces"),
		Hosts: map[string]core.HostConfig{
			"gh": {Root: "/codes/github.com"},
		},
		DefaultHost: "gh",
	}

	SetupTestConfig(t, home, cfg)

	configPath := filepath.Join(home, ".wtp.toml")
	data, err := os.ReadFile(configPath)
	if err != nil {
		t.Fatalf("read config: %v", err)
	}

	content := string(data)
	if content == "" {
		t.Fatal("config file is empty")
	}

	// Verify key fields are present
	if !strings.Contains(content, "workspace_root") {
		t.Error("config missing workspace_root")
	}
	if !strings.Contains(content, "gh") {
		t.Error("config missing host alias gh")
	}
}

func TestInitGitRepo(t *testing.T) {
	if !gitAvailable() {
		t.Skip("git not available")
	}

	dir := t.TempDir()
	repoPath := filepath.Join(dir, "test-repo")

	InitGitRepo(t, repoPath)

	// Verify .git directory exists
	gitDir := filepath.Join(repoPath, ".git")
	info, err := os.Stat(gitDir)
	if err != nil {
		t.Fatalf(".git not found: %v", err)
	}
	if !info.IsDir() {
		t.Fatal(".git is not a directory")
	}

	// Verify HEAD exists and we have at least one commit
	cmd := exec.Command("git", "rev-parse", "HEAD")
	cmd.Dir = repoPath
	out, err := cmd.CombinedOutput()
	if err != nil {
		t.Fatalf("git rev-parse HEAD failed: %v\n%s", err, out)
	}
	if len(out) < 7 {
		t.Error("HEAD hash looks too short")
	}
}

func TestInitBareGitRepo(t *testing.T) {
	if !gitAvailable() {
		t.Skip("git not available")
	}

	dir := t.TempDir()
	barePath := filepath.Join(dir, "bare-repo.git")

	InitBareGitRepo(t, barePath)

	// Bare repos have HEAD file directly (no .git subdirectory)
	headFile := filepath.Join(barePath, "HEAD")
	if _, err := os.Stat(headFile); err != nil {
		t.Fatalf("HEAD not found in bare repo: %v", err)
	}

	// Verify it reports as bare
	cmd := exec.Command("git", "rev-parse", "--is-bare-repository")
	cmd.Dir = barePath
	out, err := cmd.CombinedOutput()
	if err != nil {
		t.Fatalf("git rev-parse --is-bare-repository failed: %v\n%s", err, out)
	}
	if got := strings.TrimSpace(string(out)); got != "true" {
		t.Errorf("is-bare-repository = %q, want \"true\"", got)
	}

	// Verify it has refs (from the seeded clone)
	refsDir := filepath.Join(barePath, "refs")
	if _, err := os.Stat(refsDir); err != nil {
		t.Fatalf("refs dir not found: %v", err)
	}
}

func TestSetupTestWorkspace(t *testing.T) {
	dir := t.TempDir()

	wsPath := SetupTestWorkspace(t, dir, "my-feature")

	expectedPath := filepath.Join(dir, "my-feature")
	if wsPath != expectedPath {
		t.Errorf("wsPath = %q, want %q", wsPath, expectedPath)
	}

	// Check .wtp directory
	wtpDir := filepath.Join(wsPath, ".wtp")
	info, err := os.Stat(wtpDir)
	if err != nil {
		t.Fatalf(".wtp dir not found: %v", err)
	}
	if !info.IsDir() {
		t.Fatal(".wtp is not a directory")
	}

	// Check worktree.toml
	tomlPath := filepath.Join(wtpDir, "worktree.toml")
	if _, err := os.Stat(tomlPath); err != nil {
		t.Fatalf("worktree.toml not found: %v", err)
	}

	// Load and verify content
	wt, err := core.LoadWorktreeToml(tomlPath)
	if err != nil {
		t.Fatalf("load worktree.toml: %v", err)
	}
	if wt.Version != "1" {
		t.Errorf("Version = %q, want \"1\"", wt.Version)
	}
	if len(wt.Worktrees) != 0 {
		t.Errorf("expected empty worktrees, got %d", len(wt.Worktrees))
	}
}

func TestRunGitCmd(t *testing.T) {
	if !gitAvailable() {
		t.Skip("git not available")
	}

	dir := t.TempDir()
	InitGitRepo(t, dir)

	out := RunGitCmd(t, dir, "git", "rev-parse", "--show-toplevel")
	if out == "" {
		t.Error("RunGitCmd returned empty output")
	}
}

