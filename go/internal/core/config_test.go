package core

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestExpandTilde(t *testing.T) {
	home, err := os.UserHomeDir()
	if err != nil {
		t.Fatalf("cannot get home dir: %v", err)
	}

	tests := []struct {
		input    string
		expected string
	}{
		{"~", home},
		{"~/codes/github.com", filepath.Join(home, "codes/github.com")},
		{"/absolute/path", "/absolute/path"},
		{"relative/path", "relative/path"},
		{"", ""},
	}

	for _, tt := range tests {
		result := ExpandTilde(tt.input)
		if result != tt.expected {
			t.Errorf("ExpandTilde(%q) = %q, want %q", tt.input, result, tt.expected)
		}
	}
}

func TestDefaultGlobalConfig(t *testing.T) {
	cfg := DefaultGlobalConfig()

	if cfg.WorkspaceRoot == "" {
		t.Error("WorkspaceRoot should not be empty")
	}
	if cfg.Hosts == nil {
		t.Error("Hosts should be initialized (not nil)")
	}
	if len(cfg.Hosts) != 0 {
		t.Error("Hosts should be empty by default")
	}
	if cfg.DefaultHost != "" {
		t.Error("DefaultHost should be empty by default")
	}
	if cfg.Hooks.OnCreate != "" {
		t.Error("Hooks.OnCreate should be empty by default")
	}
}

func TestLoadConfigPriority(t *testing.T) {
	// Create a temporary directory to act as HOME.
	tmpHome := t.TempDir()

	// Override HOME for this test.
	origHome := os.Getenv("HOME")
	os.Setenv("HOME", tmpHome)
	defer os.Setenv("HOME", origHome)

	// Also set UserConfigDir-related env (for consistent behavior).
	origXDG := os.Getenv("XDG_CONFIG_HOME")
	os.Setenv("XDG_CONFIG_HOME", filepath.Join(tmpHome, ".config"))
	defer os.Setenv("XDG_CONFIG_HOME", origXDG)

	// Create config at ~/.wtp/config.toml (priority 2)
	wtpDir := filepath.Join(tmpHome, ".wtp")
	os.MkdirAll(wtpDir, 0755)
	config2 := fmt.Sprintf(`workspace_root = "%s"`, filepath.Join(tmpHome, "ws2"))
	os.WriteFile(filepath.Join(wtpDir, "config.toml"), []byte(config2), 0644)

	// Create config at ~/.wtp.toml (priority 1 - should win)
	config1 := fmt.Sprintf(`workspace_root = "%s"`, filepath.Join(tmpHome, "ws1"))
	os.WriteFile(filepath.Join(tmpHome, ".wtp.toml"), []byte(config1), 0644)

	loaded, warning, err := LoadConfig()
	if err != nil {
		t.Fatalf("LoadConfig() error: %v", err)
	}

	// Should load from ~/.wtp.toml (priority 1)
	if loaded.Config.WorkspaceRoot != filepath.Join(tmpHome, "ws1") {
		t.Errorf("expected workspace_root from priority 1 file, got %s", loaded.Config.WorkspaceRoot)
	}

	// Should warn about multiple config files
	if warning == "" {
		t.Error("expected warning about multiple config files")
	}
}

func TestLoadConfigDefault(t *testing.T) {
	// Use a temp dir with no config files.
	tmpHome := t.TempDir()

	origHome := os.Getenv("HOME")
	os.Setenv("HOME", tmpHome)
	defer os.Setenv("HOME", origHome)

	origXDG := os.Getenv("XDG_CONFIG_HOME")
	os.Setenv("XDG_CONFIG_HOME", filepath.Join(tmpHome, ".config"))
	defer os.Setenv("XDG_CONFIG_HOME", origXDG)

	loaded, warning, err := LoadConfig()
	if err != nil {
		t.Fatalf("LoadConfig() error: %v", err)
	}

	if warning != "" {
		t.Errorf("expected no warning, got %q", warning)
	}

	// Should use default config.
	if loaded.SourcePath != "" {
		t.Errorf("expected empty SourcePath for default config, got %q", loaded.SourcePath)
	}
	if loaded.Config.WorkspaceRoot == "" {
		t.Error("expected non-empty default workspace_root")
	}
}

func TestLoadConfigTildeExpansion(t *testing.T) {
	tmpHome := t.TempDir()

	origHome := os.Getenv("HOME")
	os.Setenv("HOME", tmpHome)
	defer os.Setenv("HOME", origHome)

	origXDG := os.Getenv("XDG_CONFIG_HOME")
	os.Setenv("XDG_CONFIG_HOME", filepath.Join(tmpHome, ".config"))
	defer os.Setenv("XDG_CONFIG_HOME", origXDG)

	// Create config with tilde paths.
	wtpDir := filepath.Join(tmpHome, ".wtp")
	os.MkdirAll(wtpDir, 0755)
	configContent := `
workspace_root = "~/.wtp/workspaces"

[hosts.gh]
root = "~/codes/github.com"

[hooks]
on_create = "~/.wtp/hooks/on-create.sh"
`
	os.WriteFile(filepath.Join(wtpDir, "config.toml"), []byte(configContent), 0644)

	loaded, _, err := LoadConfig()
	if err != nil {
		t.Fatalf("LoadConfig() error: %v", err)
	}

	// workspace_root should be expanded.
	expectedWsRoot := filepath.Join(tmpHome, ".wtp", "workspaces")
	if loaded.Config.WorkspaceRoot != expectedWsRoot {
		t.Errorf("workspace_root = %q, want %q", loaded.Config.WorkspaceRoot, expectedWsRoot)
	}

	// Host root should be expanded.
	ghHost, ok := loaded.Config.Hosts["gh"]
	if !ok {
		t.Fatal("expected host 'gh' to exist")
	}
	expectedHostRoot := filepath.Join(tmpHome, "codes", "github.com")
	if ghHost.Root != expectedHostRoot {
		t.Errorf("hosts.gh.root = %q, want %q", ghHost.Root, expectedHostRoot)
	}

	// Hook path should be expanded.
	expectedHook := filepath.Join(tmpHome, ".wtp", "hooks", "on-create.sh")
	if loaded.Config.Hooks.OnCreate != expectedHook {
		t.Errorf("hooks.on_create = %q, want %q", loaded.Config.Hooks.OnCreate, expectedHook)
	}
}

func TestScanWorkspaces(t *testing.T) {
	tmpDir := t.TempDir()

	cfg := GlobalConfig{
		WorkspaceRoot: tmpDir,
		Hosts:         make(map[string]HostConfig),
	}

	// Create some workspace directories.
	ws1 := filepath.Join(tmpDir, "feature-a")
	os.MkdirAll(filepath.Join(ws1, WtpDir), 0755)

	ws2 := filepath.Join(tmpDir, "feature-b")
	os.MkdirAll(filepath.Join(ws2, WtpDir), 0755)

	// Create a non-workspace directory (no .wtp).
	os.MkdirAll(filepath.Join(tmpDir, "not-a-workspace"), 0755)

	workspaces := cfg.ScanWorkspaces()

	if len(workspaces) != 2 {
		t.Fatalf("expected 2 workspaces, got %d", len(workspaces))
	}

	if _, ok := workspaces["feature-a"]; !ok {
		t.Error("expected 'feature-a' in workspaces")
	}
	if _, ok := workspaces["feature-b"]; !ok {
		t.Error("expected 'feature-b' in workspaces")
	}
	if _, ok := workspaces["not-a-workspace"]; ok {
		t.Error("'not-a-workspace' should not be in workspaces")
	}
}

func TestGetWorkspacePath(t *testing.T) {
	tmpDir := t.TempDir()

	cfg := GlobalConfig{
		WorkspaceRoot: tmpDir,
		Hosts:         make(map[string]HostConfig),
	}

	// Create a valid workspace.
	wsPath := filepath.Join(tmpDir, "my-ws")
	os.MkdirAll(filepath.Join(wsPath, WtpDir), 0755)

	path, ok := cfg.GetWorkspacePath("my-ws")
	if !ok {
		t.Error("expected GetWorkspacePath to return true for existing workspace")
	}
	if path != wsPath {
		t.Errorf("GetWorkspacePath = %q, want %q", path, wsPath)
	}

	// Non-existent workspace.
	_, ok = cfg.GetWorkspacePath("nonexistent")
	if ok {
		t.Error("expected GetWorkspacePath to return false for non-existent workspace")
	}
}

func TestGetHostRoot(t *testing.T) {
	cfg := GlobalConfig{
		Hosts: map[string]HostConfig{
			"gh": {Root: "/home/user/codes/github.com"},
			"gl": {Root: "/home/user/codes/gitlab.com"},
		},
	}

	root, ok := cfg.GetHostRoot("gh")
	if !ok || root != "/home/user/codes/github.com" {
		t.Errorf("GetHostRoot(gh) = (%q, %v), want (/home/user/codes/github.com, true)", root, ok)
	}

	_, ok = cfg.GetHostRoot("bb")
	if ok {
		t.Error("GetHostRoot(bb) should return false")
	}
}

func TestSaveConfig(t *testing.T) {
	tmpDir := t.TempDir()
	configPath := filepath.Join(tmpDir, "config.toml")

	lc := &LoadedConfig{
		Config: GlobalConfig{
			WorkspaceRoot: "/tmp/workspaces",
			Hosts: map[string]HostConfig{
				"gh": {Root: "/home/user/codes/github.com"},
			},
			DefaultHost: "gh",
		},
		SourcePath: configPath,
	}

	if err := lc.Save(); err != nil {
		t.Fatalf("Save() error: %v", err)
	}

	// Verify the file was written.
	data, err := os.ReadFile(configPath)
	if err != nil {
		t.Fatalf("failed to read saved config: %v", err)
	}

	content := string(data)
	if !strings.Contains(content, "workspace_root") {
		t.Error("saved config should contain workspace_root")
	}
	if !strings.Contains(content, "github.com") {
		t.Error("saved config should contain host root")
	}
}

func TestResolveWorkspacePath(t *testing.T) {
	cfg := GlobalConfig{
		WorkspaceRoot: "/home/user/.wtp/workspaces",
	}

	result := cfg.ResolveWorkspacePath("feature-x")
	expected := filepath.Join("/home/user/.wtp/workspaces", "feature-x")
	if result != expected {
		t.Errorf("ResolveWorkspacePath = %q, want %q", result, expected)
	}
}
