package core

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"sort"
	"strings"
)

// WorkspaceInfo holds summary information about a workspace.
type WorkspaceInfo struct {
	Name   string
	Path   string
	Exists bool
}

// WorkspaceManager manages workspaces and their discovery.
type WorkspaceManager struct {
	loadedConfig *LoadedConfig
}

// NewWorkspaceManager creates a new WorkspaceManager.
func NewWorkspaceManager(cfg *LoadedConfig) *WorkspaceManager {
	return &WorkspaceManager{loadedConfig: cfg}
}

// GlobalConfig returns the underlying GlobalConfig.
func (m *WorkspaceManager) GlobalConfig() *GlobalConfig {
	return &m.loadedConfig.Config
}

// LoadedConfig returns the underlying LoadedConfig.
func (m *WorkspaceManager) LoadedConfig() *LoadedConfig {
	return m.loadedConfig
}

// ListWorkspaces returns all workspaces found in workspace_root.
// Results are sorted by name.
func (m *WorkspaceManager) ListWorkspaces() []WorkspaceInfo {
	scanned := m.loadedConfig.ScanWorkspaces()
	workspaces := make([]WorkspaceInfo, 0, len(scanned))
	for name, path := range scanned {
		workspaces = append(workspaces, WorkspaceInfo{
			Name:   name,
			Path:   path,
			Exists: true,
		})
	}
	sort.Slice(workspaces, func(i, j int) bool {
		return workspaces[i].Name < workspaces[j].Name
	})
	return workspaces
}

// CreateWorkspace creates a new workspace directory with .wtp metadata.
// If runHook is true and a hook is configured, it will be executed.
func (m *WorkspaceManager) CreateWorkspace(name string, runHook bool) (string, error) {
	workspacePath := m.GlobalConfig().ResolveWorkspacePath(name)

	// Check if workspace already exists (directory with .wtp subdirectory).
	wtpDir := filepath.Join(workspacePath, WtpDir)
	if info, err := os.Stat(wtpDir); err == nil && info.IsDir() {
		return "", &WorkspaceAlreadyExistsError{
			Name: name,
			Path: workspacePath,
		}
	}

	// Check if directory exists but is not a workspace.
	if info, err := os.Stat(workspacePath); err == nil && info.IsDir() {
		return "", NewConfigError(fmt.Sprintf(
			"directory '%s' already exists but is not a wtp workspace",
			workspacePath,
		))
	}

	// Create workspace directory structure with fence protection.
	fence := NewFenceFromConfig(m.GlobalConfig())
	if err := m.initializeWorkspaceDir(workspacePath, fence); err != nil {
		return "", err
	}

	// Run post-create hook if configured and enabled.
	if runHook {
		if err := m.runCreateHook(name, workspacePath); err != nil {
			fmt.Fprintf(os.Stderr, "Warning: Failed to run create hook: %v\n", err)
		}
	}

	return workspacePath, nil
}

// initializeWorkspaceDir creates the workspace directory structure.
func (m *WorkspaceManager) initializeWorkspaceDir(path string, fence *Fence) error {
	// Create main directory.
	if err := fence.CreateDirAll(path); err != nil {
		return err
	}

	// Create .wtp directory.
	wtpDir := filepath.Join(path, WtpDir)
	if err := fence.CreateDirAll(wtpDir); err != nil {
		return err
	}

	// Create empty worktree.toml.
	wt := NewWorktreeToml()
	worktreeTomlPath := filepath.Join(wtpDir, "worktree.toml")
	return wt.Save(worktreeTomlPath, func(p string, data []byte) error {
		return fence.Write(p, data)
	})
}

// runCreateHook executes the on_create hook script if configured.
func (m *WorkspaceManager) runCreateHook(name, path string) error {
	hookPath := m.loadedConfig.Config.Hooks.OnCreate
	if hookPath == "" {
		return nil
	}

	if _, err := os.Stat(hookPath); os.IsNotExist(err) {
		return NewConfigError(fmt.Sprintf("create hook not found: %s", hookPath))
	}

	// Check if hook is executable on Unix.
	if runtime.GOOS != "windows" {
		info, err := os.Stat(hookPath)
		if err != nil {
			return err
		}
		if info.Mode()&0111 == 0 {
			return NewConfigError(fmt.Sprintf("create hook is not executable: %s", hookPath))
		}
	}

	// Run the hook with environment variables.
	cmd := exec.Command(hookPath)
	cmd.Env = append(os.Environ(),
		"WTP_WORKSPACE_NAME="+name,
		"WTP_WORKSPACE_PATH="+path,
	)
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr

	if err := cmd.Run(); err != nil {
		if exitErr, ok := err.(*exec.ExitError); ok {
			return NewConfigError(fmt.Sprintf(
				"create hook failed with exit code %d",
				exitErr.ExitCode(),
			))
		}
		return NewConfigError(fmt.Sprintf("failed to execute create hook: %v", err))
	}

	return nil
}

// RemoveWorkspace removes a workspace directory.
// The caller is responsible for ejecting worktrees before calling this.
func (m *WorkspaceManager) RemoveWorkspace(name string, deleteDir bool) (string, error) {
	path, ok := m.GlobalConfig().GetWorkspacePath(name)
	if !ok {
		return "", nil
	}

	if deleteDir {
		if info, err := os.Stat(path); err == nil && info.IsDir() {
			fence := EnsureFence(m.GlobalConfig())
			if err := fence.RemoveDirAll(path); err != nil {
				return path, err
			}
		}
	}

	return path, nil
}

// DetectCurrentWorkspace walks up from cwd looking for a directory with .wtp/.
// Returns (workspace_name, workspace_path, error).
func (m *WorkspaceManager) DetectCurrentWorkspace(cwd string) (string, string, error) {
	checkDir, err := filepath.Abs(cwd)
	if err != nil {
		return "", "", err
	}

	for {
		wtpDir := filepath.Join(checkDir, WtpDir)
		info, err := os.Stat(wtpDir)
		if err == nil && info.IsDir() {
			// Found a .wtp directory. Check if it's a registered workspace.
			resolvedCheck := resolvePath(checkDir)
			scanned := m.GlobalConfig().ScanWorkspaces()
			for name, wsPath := range scanned {
				if resolvePath(wsPath) == resolvedCheck {
					return name, wsPath, nil
				}
			}
			// Directory has .wtp but is not registered - use directory name.
			name := filepath.Base(checkDir)
			if name == "." || name == string(filepath.Separator) {
				name = "workspace"
			}
			return name, checkDir, nil
		}

		// Move up to parent.
		parent := filepath.Dir(checkDir)
		if parent == checkDir {
			break
		}
		checkDir = parent
	}

	return "", "", fmt.Errorf(
		"not in a workspace directory.\n" +
			"Run this command from within a workspace directory.",
	)
}

// MatchHostAlias tries to match a repository path to a host alias.
// Returns a Hosted RepoRef if matched, or nil if no match.
func (m *WorkspaceManager) MatchHostAlias(repoPath string) *RepoRef {
	absRepo := resolvePath(repoPath)

	for alias, hostCfg := range m.GlobalConfig().Hosts {
		absHost := resolvePath(hostCfg.Root)
		rel, err := filepath.Rel(absHost, absRepo)
		if err != nil {
			continue
		}
		// If the relative path doesn't start with "..", it's inside this host root.
		if !strings.HasPrefix(rel, "..") && rel != "." {
			ref := NewHostedRepoRef(alias, filepath.ToSlash(rel))
			return &ref
		}
	}
	return nil
}

// GetHosts returns all configured host aliases.
func (m *WorkspaceManager) GetHosts() map[string]HostConfig {
	return m.GlobalConfig().Hosts
}
