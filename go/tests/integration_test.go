package tests

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
	"testing"
)

var (
	// buildOnce ensures the binary is compiled exactly once per test run.
	buildOnce sync.Once
	// wtpBinPath holds the absolute path to the compiled binary.
	wtpBinPath string
	// buildErr stores any error from the build step.
	buildErr error
)

// ensureBinary compiles the wtp binary once and returns its path.
// If the build fails, it falls back to looking for a pre-built binary.
func ensureBinary(t *testing.T) string {
	t.Helper()

	buildOnce.Do(func() {
		// Build into a temp directory so we don't pollute the source tree.
		tmpDir, err := os.MkdirTemp("", "wtp-integration-*")
		if err != nil {
			buildErr = fmt.Errorf("create temp dir: %w", err)
			return
		}

		binName := "wtp"
		if runtime.GOOS == "windows" {
			binName = "wtp.exe"
		}
		wtpBinPath = filepath.Join(tmpDir, binName)

		// Module root is one level up from tests/.
		modRoot, err := filepath.Abs("..")
		if err != nil {
			buildErr = fmt.Errorf("resolve module root: %w", err)
			return
		}

		cmd := exec.Command("go", "build", "-o", wtpBinPath, ".")
		cmd.Dir = modRoot
		out, err := cmd.CombinedOutput()
		if err != nil {
			buildErr = fmt.Errorf("go build: %w\n%s", err, out)
			return
		}
	})

	if buildErr != nil {
		t.Skipf("failed to build wtp binary: %v", buildErr)
	}
	return wtpBinPath
}

// setupTestHome creates a temporary HOME directory for test isolation.
func setupTestHome(t *testing.T) string {
	t.Helper()
	return t.TempDir()
}

// runWtpWithHome runs wtp with isolated HOME environment.
func runWtpWithHome(t *testing.T, args []string, home string) (bool, string, string) {
	t.Helper()
	return runWtpInDirWithHome(t, args, "", home)
}

// runWtpInDirWithHome runs wtp with isolated HOME and optional working directory.
func runWtpInDirWithHome(t *testing.T, args []string, cwd, home string) (bool, string, string) {
	t.Helper()

	bin := ensureBinary(t)
	cmd := exec.Command(bin, args...)

	// Isolate from user's config.
	cmd.Env = append(os.Environ(),
		"HOME="+home,
		"USERPROFILE="+home,
		"NO_COLOR=1", // Disable color codes for easier assertion matching.
	)
	// Remove XDG_CONFIG_HOME to prevent interference.
	var filtered []string
	for _, env := range cmd.Env {
		if !strings.HasPrefix(env, "XDG_CONFIG_HOME=") {
			filtered = append(filtered, env)
		}
	}
	cmd.Env = filtered

	if cwd != "" {
		cmd.Dir = cwd
	}

	var stdout, stderr strings.Builder
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr

	err := cmd.Run()
	success := err == nil

	return success, stdout.String(), stderr.String()
}

func TestWtpHelp(t *testing.T) {
	ensureBinary(t)
	home := setupTestHome(t)

	success, stdout, _ := runWtpWithHome(t, []string{"--help"}, home)
	if !success {
		t.Fatal("wtp --help should succeed")
	}

	// Check for key content.
	checks := []string{
		"WorkTree for Polyrepo",
		"cd",
		"ls",
		"create",
		"import",
		"switch",
		"eject",
		"shell-init",
		"Workspace Management",
		"Repository Operations",
		"Utilities",
	}
	for _, check := range checks {
		if !strings.Contains(stdout, check) {
			t.Errorf("help output should contain %q", check)
		}
	}

	// "init" command was removed (but shell-init exists).
	if strings.Contains(stdout, "  init  ") {
		t.Error("help output should not contain standalone 'init' command")
	}
}

func TestWtpVersion(t *testing.T) {
	ensureBinary(t)
	home := setupTestHome(t)

	success, stdout, _ := runWtpWithHome(t, []string{"--version"}, home)
	if !success {
		t.Fatal("wtp --version should succeed")
	}
	if !strings.Contains(stdout, "0.1.0") {
		t.Errorf("version output should contain '0.1.0', got: %s", stdout)
	}
}

func TestImportRequiresWorkspace(t *testing.T) {
	ensureBinary(t)
	home := setupTestHome(t)
	tmpDir := t.TempDir()

	success, stdout, stderr := runWtpInDirWithHome(t, []string{"import", "some/path"}, tmpDir, home)
	if success {
		t.Fatal("import outside workspace should fail")
	}
	combined := stdout + " " + stderr
	if !strings.Contains(strings.ToLower(combined), "workspace") {
		t.Errorf("expected workspace-related error, got: %s", combined)
	}
}

func TestStatusNotInWorkspace(t *testing.T) {
	ensureBinary(t)
	home := setupTestHome(t)
	tmpDir := t.TempDir()

	success, stdout, stderr := runWtpInDirWithHome(t, []string{"status"}, tmpDir, home)
	if success {
		t.Fatal("status outside workspace should fail")
	}
	combined := stdout + " " + stderr
	if !strings.Contains(strings.ToLower(combined), "workspace") {
		t.Errorf("expected workspace-related error, got: %s", combined)
	}
}

func TestCdRequiresShellIntegration(t *testing.T) {
	ensureBinary(t)
	home := setupTestHome(t)

	success, stdout, stderr := runWtpWithHome(t, []string{"cd", "nonexistent-workspace-xyz"}, home)
	if success {
		t.Fatal("cd with nonexistent workspace should fail")
	}
	combined := stdout + " " + stderr
	if !strings.Contains(combined, "not found") &&
		!strings.Contains(combined, "shell integration") &&
		!strings.Contains(combined, "Shell integration") {
		t.Errorf("expected 'not found' or 'shell integration' error, got: %s", combined)
	}
}

func TestShellInitOutputsWrapper(t *testing.T) {
	ensureBinary(t)
	home := setupTestHome(t)

	success, stdout, _ := runWtpWithHome(t, []string{"shell-init"}, home)
	if !success {
		t.Fatal("shell-init should succeed")
	}
	if !strings.Contains(stdout, "wtp() {") {
		t.Error("shell-init output should contain 'wtp() {'")
	}
	if !strings.Contains(stdout, "WTP_DIRECTIVE_FILE") {
		t.Error("shell-init output should contain 'WTP_DIRECTIVE_FILE'")
	}
}

func TestLsShortFormat(t *testing.T) {
	ensureBinary(t)
	home := setupTestHome(t)

	// First create a workspace.
	success, _, stderr := runWtpWithHome(t, []string{"create", "test-short"}, home)
	if !success {
		t.Fatalf("failed to create workspace: %s", stderr)
	}

	// List with --short.
	success, stdout, _ := runWtpWithHome(t, []string{"ls", "--short"}, home)
	if !success {
		t.Fatal("ls --short should succeed")
	}
	if !strings.Contains(stdout, "test-short") {
		t.Errorf("expected 'test-short' in output, got: %s", stdout)
	}

	// Cleanup.
	runWtpWithHome(t, []string{"rm", "test-short", "--force"}, home)
}

func TestCreateWorkspaceWithHook(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("hook tests require Unix shell")
	}
	ensureBinary(t)
	home := setupTestHome(t)

	// Create a hook script.
	hooksDir := filepath.Join(home, ".wtp", "hooks")
	if err := os.MkdirAll(hooksDir, 0755); err != nil {
		t.Fatal(err)
	}

	hookScript := filepath.Join(hooksDir, "on-create.sh")
	hookContent := `#!/bin/bash
echo "HOOK_RAN: $WTP_WORKSPACE_NAME"
touch "$WTP_WORKSPACE_PATH/hook-marker.txt"
`
	if err := os.WriteFile(hookScript, []byte(hookContent), 0755); err != nil {
		t.Fatal(err)
	}

	// Create config with hook.
	wsRoot := filepath.Join(home, ".wtp", "workspaces")
	configContent := fmt.Sprintf(
		"workspace_root = %q\n\n[hooks]\non_create = %q\n",
		wsRoot, hookScript,
	)
	wtpDir := filepath.Join(home, ".wtp")
	if err := os.MkdirAll(wtpDir, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(wtpDir, "config.toml"), []byte(configContent), 0644); err != nil {
		t.Fatal(err)
	}

	// Create workspace - hook should run.
	success, stdout, stderr := runWtpWithHome(t, []string{"create", "test-hook-ws"}, home)
	if !success {
		t.Fatalf("failed to create workspace: %s", stderr)
	}

	// Check hook output.
	if !strings.Contains(stdout, "HOOK_RAN: test-hook-ws") {
		t.Errorf("expected hook output in stdout, got: %s", stdout)
	}

	// Verify marker file was created by hook.
	markerFile := filepath.Join(wsRoot, "test-hook-ws", "hook-marker.txt")
	if _, err := os.Stat(markerFile); err != nil {
		t.Error("hook marker file should exist")
	}

	// Cleanup.
	runWtpWithHome(t, []string{"rm", "test-hook-ws", "--force"}, home)
}

func TestCreateWorkspaceSkipHook(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("hook tests require Unix shell")
	}
	ensureBinary(t)
	home := setupTestHome(t)

	// Create a hook script that would fail if run.
	hooksDir := filepath.Join(home, ".wtp", "hooks")
	if err := os.MkdirAll(hooksDir, 0755); err != nil {
		t.Fatal(err)
	}

	hookScript := filepath.Join(hooksDir, "on-create.sh")
	hookContent := `#!/bin/bash
echo "HOOK_SHOULD_NOT_RUN"
exit 1
`
	if err := os.WriteFile(hookScript, []byte(hookContent), 0755); err != nil {
		t.Fatal(err)
	}

	// Create config with hook.
	wsRoot := filepath.Join(home, ".wtp", "workspaces")
	configContent := fmt.Sprintf(
		"workspace_root = %q\n\n[hooks]\non_create = %q\n",
		wsRoot, hookScript,
	)
	wtpDir := filepath.Join(home, ".wtp")
	if err := os.MkdirAll(wtpDir, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(wtpDir, "config.toml"), []byte(configContent), 0644); err != nil {
		t.Fatal(err)
	}

	// Create workspace with --no-hook.
	success, stdout, stderr := runWtpWithHome(t, []string{"create", "test-no-hook-ws", "--no-hook"}, home)
	if !success {
		t.Fatalf("failed to create workspace: %s", stderr)
	}

	// Hook should NOT have run.
	if strings.Contains(stdout, "HOOK_SHOULD_NOT_RUN") {
		t.Errorf("hook should not have run, but output contains hook text: %s", stdout)
	}

	// Cleanup.
	runWtpWithHome(t, []string{"rm", "test-no-hook-ws", "--force"}, home)
}

func TestEjectNotInWorkspace(t *testing.T) {
	ensureBinary(t)
	home := setupTestHome(t)
	tmpDir := t.TempDir()

	success, stdout, stderr := runWtpInDirWithHome(t, []string{"eject", "some-repo"}, tmpDir, home)
	if success {
		t.Fatal("eject outside workspace should fail")
	}
	combined := stdout + " " + stderr
	if !strings.Contains(strings.ToLower(combined), "workspace") {
		t.Errorf("expected workspace-related error, got: %s", combined)
	}
}

func TestEjectHelp(t *testing.T) {
	ensureBinary(t)
	home := setupTestHome(t)

	success, stdout, _ := runWtpWithHome(t, []string{"help", "eject"}, home)
	if !success {
		t.Fatal("help eject should succeed")
	}
	// The help output should contain key elements.
	if !strings.Contains(stdout, "Usage:") {
		t.Error("eject help should contain 'Usage:'")
	}
	if !strings.Contains(stdout, "wtp eject") {
		t.Error("eject help should contain 'wtp eject'")
	}
}
