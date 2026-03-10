package core

import (
	"os"
	"path/filepath"
	"testing"
)

func TestCreateWorkspace(t *testing.T) {
	tmpDir := t.TempDir()

	cfg := &LoadedConfig{
		Config: GlobalConfig{
			WorkspaceRoot: tmpDir,
			Hosts:         make(map[string]HostConfig),
		},
	}
	mgr := NewWorkspaceManager(cfg)

	path, err := mgr.CreateWorkspace("test-ws", false)
	if err != nil {
		t.Fatalf("CreateWorkspace() error: %v", err)
	}

	expected := filepath.Join(tmpDir, "test-ws")
	if path != expected {
		t.Errorf("CreateWorkspace() path = %q, want %q", path, expected)
	}

	// Verify workspace directory exists.
	info, err := os.Stat(path)
	if err != nil {
		t.Fatalf("workspace directory not created: %v", err)
	}
	if !info.IsDir() {
		t.Error("expected a directory")
	}

	// Verify .wtp directory exists.
	wtpDir := filepath.Join(path, WtpDir)
	info, err = os.Stat(wtpDir)
	if err != nil {
		t.Fatalf(".wtp directory not created: %v", err)
	}
	if !info.IsDir() {
		t.Error("expected .wtp to be a directory")
	}

	// Verify worktree.toml exists and is valid.
	worktreeTomlPath := filepath.Join(wtpDir, "worktree.toml")
	wt, err := LoadWorktreeToml(worktreeTomlPath)
	if err != nil {
		t.Fatalf("failed to load worktree.toml: %v", err)
	}
	if wt.Version != "1" {
		t.Errorf("worktree.toml version = %q, want %q", wt.Version, "1")
	}
	if len(wt.Worktrees) != 0 {
		t.Errorf("worktree.toml should have 0 entries, got %d", len(wt.Worktrees))
	}
}

func TestCreateWorkspaceDuplicate(t *testing.T) {
	tmpDir := t.TempDir()

	cfg := &LoadedConfig{
		Config: GlobalConfig{
			WorkspaceRoot: tmpDir,
			Hosts:         make(map[string]HostConfig),
		},
	}
	mgr := NewWorkspaceManager(cfg)

	// Create workspace first time.
	_, err := mgr.CreateWorkspace("dup-ws", false)
	if err != nil {
		t.Fatalf("first CreateWorkspace() error: %v", err)
	}

	// Creating again should fail with WorkspaceAlreadyExistsError.
	_, err = mgr.CreateWorkspace("dup-ws", false)
	if err == nil {
		t.Fatal("expected error on duplicate creation, got nil")
	}

	if _, ok := err.(*WorkspaceAlreadyExistsError); !ok {
		t.Errorf("expected WorkspaceAlreadyExistsError, got %T: %v", err, err)
	}
}

func TestCreateWorkspaceDirExistsNotWorkspace(t *testing.T) {
	tmpDir := t.TempDir()

	// Create a plain directory (no .wtp inside).
	plainDir := filepath.Join(tmpDir, "plain-dir")
	os.MkdirAll(plainDir, 0755)

	cfg := &LoadedConfig{
		Config: GlobalConfig{
			WorkspaceRoot: tmpDir,
			Hosts:         make(map[string]HostConfig),
		},
	}
	mgr := NewWorkspaceManager(cfg)

	_, err := mgr.CreateWorkspace("plain-dir", false)
	if err == nil {
		t.Fatal("expected error when directory exists but is not a workspace")
	}

	if _, ok := err.(*ConfigError); !ok {
		t.Errorf("expected ConfigError, got %T: %v", err, err)
	}
}

func TestListWorkspaces(t *testing.T) {
	tmpDir := t.TempDir()

	// Create workspace directories manually.
	for _, name := range []string{"alpha", "beta", "gamma"} {
		wsDir := filepath.Join(tmpDir, name)
		os.MkdirAll(filepath.Join(wsDir, WtpDir), 0755)
	}

	// Create a non-workspace directory.
	os.MkdirAll(filepath.Join(tmpDir, "not-ws"), 0755)

	cfg := &LoadedConfig{
		Config: GlobalConfig{
			WorkspaceRoot: tmpDir,
			Hosts:         make(map[string]HostConfig),
		},
	}
	mgr := NewWorkspaceManager(cfg)

	workspaces := mgr.ListWorkspaces()

	if len(workspaces) != 3 {
		t.Fatalf("expected 3 workspaces, got %d", len(workspaces))
	}

	// Should be sorted by name.
	if workspaces[0].Name != "alpha" {
		t.Errorf("workspaces[0].Name = %q, want %q", workspaces[0].Name, "alpha")
	}
	if workspaces[1].Name != "beta" {
		t.Errorf("workspaces[1].Name = %q, want %q", workspaces[1].Name, "beta")
	}
	if workspaces[2].Name != "gamma" {
		t.Errorf("workspaces[2].Name = %q, want %q", workspaces[2].Name, "gamma")
	}

	for _, ws := range workspaces {
		if !ws.Exists {
			t.Errorf("workspace %q should have Exists=true", ws.Name)
		}
	}
}

func TestRemoveWorkspace(t *testing.T) {
	tmpDir := t.TempDir()

	cfg := &LoadedConfig{
		Config: GlobalConfig{
			WorkspaceRoot: tmpDir,
			Hosts:         make(map[string]HostConfig),
		},
	}
	mgr := NewWorkspaceManager(cfg)

	// Create a workspace.
	_, err := mgr.CreateWorkspace("to-remove", false)
	if err != nil {
		t.Fatalf("CreateWorkspace() error: %v", err)
	}

	wsPath := filepath.Join(tmpDir, "to-remove")

	// Verify it exists.
	if _, err := os.Stat(wsPath); err != nil {
		t.Fatalf("workspace should exist: %v", err)
	}

	// Remove with deleteDir=true.
	path, err := mgr.RemoveWorkspace("to-remove", true)
	if err != nil {
		t.Fatalf("RemoveWorkspace() error: %v", err)
	}
	if path != wsPath {
		t.Errorf("RemoveWorkspace() path = %q, want %q", path, wsPath)
	}

	// Verify directory is gone.
	if _, err := os.Stat(wsPath); !os.IsNotExist(err) {
		t.Error("workspace directory should be removed")
	}
}

func TestRemoveWorkspaceKeepDir(t *testing.T) {
	tmpDir := t.TempDir()

	cfg := &LoadedConfig{
		Config: GlobalConfig{
			WorkspaceRoot: tmpDir,
			Hosts:         make(map[string]HostConfig),
		},
	}
	mgr := NewWorkspaceManager(cfg)

	_, err := mgr.CreateWorkspace("keep-dir", false)
	if err != nil {
		t.Fatalf("CreateWorkspace() error: %v", err)
	}

	wsPath := filepath.Join(tmpDir, "keep-dir")

	// Remove with deleteDir=false.
	path, err := mgr.RemoveWorkspace("keep-dir", false)
	if err != nil {
		t.Fatalf("RemoveWorkspace() error: %v", err)
	}
	if path != wsPath {
		t.Errorf("RemoveWorkspace() path = %q, want %q", path, wsPath)
	}

	// Directory should still exist.
	if _, err := os.Stat(wsPath); err != nil {
		t.Errorf("workspace directory should still exist: %v", err)
	}
}

func TestRemoveWorkspaceNotFound(t *testing.T) {
	tmpDir := t.TempDir()

	cfg := &LoadedConfig{
		Config: GlobalConfig{
			WorkspaceRoot: tmpDir,
			Hosts:         make(map[string]HostConfig),
		},
	}
	mgr := NewWorkspaceManager(cfg)

	path, err := mgr.RemoveWorkspace("nonexistent", true)
	if err != nil {
		t.Fatalf("RemoveWorkspace() error: %v", err)
	}
	if path != "" {
		t.Errorf("expected empty path for non-existent workspace, got %q", path)
	}
}

func TestDetectCurrentWorkspace(t *testing.T) {
	tmpDir := t.TempDir()

	cfg := &LoadedConfig{
		Config: GlobalConfig{
			WorkspaceRoot: tmpDir,
			Hosts:         make(map[string]HostConfig),
		},
	}
	mgr := NewWorkspaceManager(cfg)

	// Create a workspace.
	_, err := mgr.CreateWorkspace("detect-me", false)
	if err != nil {
		t.Fatalf("CreateWorkspace() error: %v", err)
	}

	wsPath := filepath.Join(tmpDir, "detect-me")

	// Detect from the workspace root itself.
	name, path, err := mgr.DetectCurrentWorkspace(wsPath)
	if err != nil {
		t.Fatalf("DetectCurrentWorkspace() error: %v", err)
	}
	if name != "detect-me" {
		t.Errorf("name = %q, want %q", name, "detect-me")
	}
	if path != wsPath {
		t.Errorf("path = %q, want %q", path, wsPath)
	}
}

func TestDetectCurrentWorkspaceFromSubdir(t *testing.T) {
	tmpDir := t.TempDir()

	cfg := &LoadedConfig{
		Config: GlobalConfig{
			WorkspaceRoot: tmpDir,
			Hosts:         make(map[string]HostConfig),
		},
	}
	mgr := NewWorkspaceManager(cfg)

	_, err := mgr.CreateWorkspace("ws-sub", false)
	if err != nil {
		t.Fatalf("CreateWorkspace() error: %v", err)
	}

	// Create a subdirectory inside the workspace.
	subDir := filepath.Join(tmpDir, "ws-sub", "repo", "src")
	os.MkdirAll(subDir, 0755)

	// Detect from the subdirectory - should walk up and find the workspace.
	name, _, err := mgr.DetectCurrentWorkspace(subDir)
	if err != nil {
		t.Fatalf("DetectCurrentWorkspace() error: %v", err)
	}
	if name != "ws-sub" {
		t.Errorf("name = %q, want %q", name, "ws-sub")
	}
}

func TestDetectCurrentWorkspaceNotFound(t *testing.T) {
	tmpDir := t.TempDir()

	cfg := &LoadedConfig{
		Config: GlobalConfig{
			WorkspaceRoot: tmpDir,
			Hosts:         make(map[string]HostConfig),
		},
	}
	mgr := NewWorkspaceManager(cfg)

	// Detect from a directory that is not a workspace.
	_, _, err := mgr.DetectCurrentWorkspace(tmpDir)
	if err == nil {
		t.Fatal("expected error when not in a workspace")
	}
}

func TestMatchHostAlias(t *testing.T) {
	tmpDir := t.TempDir()

	// Create a host root directory.
	hostRoot := filepath.Join(tmpDir, "github.com")
	os.MkdirAll(hostRoot, 0755)

	cfg := &LoadedConfig{
		Config: GlobalConfig{
			WorkspaceRoot: tmpDir,
			Hosts: map[string]HostConfig{
				"gh": {Root: hostRoot},
			},
		},
	}
	mgr := NewWorkspaceManager(cfg)

	// A path inside the host root should match.
	repoPath := filepath.Join(hostRoot, "owner", "repo")
	ref := mgr.MatchHostAlias(repoPath)
	if ref == nil {
		t.Fatal("expected a RepoRef, got nil")
	}
	if ref.Kind != RepoRefHosted {
		t.Errorf("expected RepoRefHosted, got %d", ref.Kind)
	}
	if ref.Host != "gh" {
		t.Errorf("Host = %q, want %q", ref.Host, "gh")
	}
	if ref.Path != "owner/repo" {
		t.Errorf("Path = %q, want %q", ref.Path, "owner/repo")
	}
}

func TestMatchHostAliasNoMatch(t *testing.T) {
	tmpDir := t.TempDir()

	hostRoot := filepath.Join(tmpDir, "github.com")
	os.MkdirAll(hostRoot, 0755)

	cfg := &LoadedConfig{
		Config: GlobalConfig{
			WorkspaceRoot: tmpDir,
			Hosts: map[string]HostConfig{
				"gh": {Root: hostRoot},
			},
		},
	}
	mgr := NewWorkspaceManager(cfg)

	// A path outside all host roots should return nil.
	outsideDir := t.TempDir()
	ref := mgr.MatchHostAlias(filepath.Join(outsideDir, "some", "repo"))
	if ref != nil {
		t.Errorf("expected nil for path outside host roots, got %+v", ref)
	}
}

func TestGetHosts(t *testing.T) {
	cfg := &LoadedConfig{
		Config: GlobalConfig{
			Hosts: map[string]HostConfig{
				"gh": {Root: "/codes/github.com"},
				"gl": {Root: "/codes/gitlab.com"},
			},
		},
	}
	mgr := NewWorkspaceManager(cfg)

	hosts := mgr.GetHosts()
	if len(hosts) != 2 {
		t.Fatalf("expected 2 hosts, got %d", len(hosts))
	}
	if hosts["gh"].Root != "/codes/github.com" {
		t.Errorf("hosts[gh].Root = %q, want %q", hosts["gh"].Root, "/codes/github.com")
	}
	if hosts["gl"].Root != "/codes/gitlab.com" {
		t.Errorf("hosts[gl].Root = %q, want %q", hosts["gl"].Root, "/codes/gitlab.com")
	}
}

func TestRunCreateHookNoHook(t *testing.T) {
	tmpDir := t.TempDir()

	cfg := &LoadedConfig{
		Config: GlobalConfig{
			WorkspaceRoot: tmpDir,
			Hosts:         make(map[string]HostConfig),
			Hooks:         HooksConfig{}, // No hook configured
		},
	}
	mgr := NewWorkspaceManager(cfg)

	// runCreateHook should return nil when no hook is configured.
	err := mgr.runCreateHook("test", filepath.Join(tmpDir, "test"))
	if err != nil {
		t.Errorf("runCreateHook() with no hook should return nil, got: %v", err)
	}
}

func TestRunCreateHookNotFound(t *testing.T) {
	tmpDir := t.TempDir()

	cfg := &LoadedConfig{
		Config: GlobalConfig{
			WorkspaceRoot: tmpDir,
			Hosts:         make(map[string]HostConfig),
			Hooks: HooksConfig{
				OnCreate: filepath.Join(tmpDir, "nonexistent-hook.sh"),
			},
		},
	}
	mgr := NewWorkspaceManager(cfg)

	err := mgr.runCreateHook("test", filepath.Join(tmpDir, "test"))
	if err == nil {
		t.Fatal("expected error for missing hook file, got nil")
	}

	if _, ok := err.(*ConfigError); !ok {
		t.Errorf("expected ConfigError, got %T: %v", err, err)
	}
}
