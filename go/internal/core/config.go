package core

import (
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strings"

	"github.com/BurntSushi/toml"
)

// DefaultWorkspaceRoot is the default directory name for workspaces under home.
const DefaultWorkspaceRoot = ".wtp/workspaces"

// WtpDir is the metadata directory name inside each workspace.
const WtpDir = ".wtp"

// HostConfig represents a host alias mapping to a root directory.
type HostConfig struct {
	Root string `toml:"root"`
}

// HooksConfig holds hook script paths for workspace lifecycle events.
type HooksConfig struct {
	// OnCreate is the path to a script run after creating a workspace.
	// Environment variables: WTP_WORKSPACE_NAME, WTP_WORKSPACE_PATH
	OnCreate string `toml:"on_create,omitempty"`
}

// GlobalConfig is the serializable configuration structure.
type GlobalConfig struct {
	// WorkspaceRoot is the root directory for all workspaces.
	WorkspaceRoot string `toml:"workspace_root"`

	// Hosts maps host alias names to their root directories.
	Hosts map[string]HostConfig `toml:"hosts,omitempty"`

	// DefaultHost is the default host alias to use when not specified.
	DefaultHost string `toml:"default_host,omitempty"`

	// Hooks contains hook configuration for workspace lifecycle events.
	Hooks HooksConfig `toml:"hooks,omitempty"`
}

// DefaultGlobalConfig returns a GlobalConfig with default values.
func DefaultGlobalConfig() GlobalConfig {
	return GlobalConfig{
		WorkspaceRoot: defaultWorkspaceRoot(),
		Hosts:         make(map[string]HostConfig),
	}
}

// GetWorkspacePath returns the path for a workspace by name if it exists
// (directory with .wtp subdirectory).
func (c *GlobalConfig) GetWorkspacePath(name string) (string, bool) {
	p := filepath.Join(c.WorkspaceRoot, name)
	wtpDir := filepath.Join(p, WtpDir)
	info, err := os.Stat(p)
	if err != nil || !info.IsDir() {
		return "", false
	}
	wtpInfo, err := os.Stat(wtpDir)
	if err != nil || !wtpInfo.IsDir() {
		return "", false
	}
	return p, true
}

// ScanWorkspaces scans workspace_root for directories containing a .wtp subdirectory.
// Returns a map of workspace name to absolute path.
func (c *GlobalConfig) ScanWorkspaces() map[string]string {
	workspaces := make(map[string]string)

	entries, err := os.ReadDir(c.WorkspaceRoot)
	if err != nil {
		return workspaces
	}

	for _, entry := range entries {
		if !entry.IsDir() {
			continue
		}
		p := filepath.Join(c.WorkspaceRoot, entry.Name())
		wtpDir := filepath.Join(p, WtpDir)
		info, err := os.Stat(wtpDir)
		if err == nil && info.IsDir() {
			workspaces[entry.Name()] = p
		}
	}

	return workspaces
}

// GetHostRoot returns the root directory for a host alias, or empty string if not found.
func (c *GlobalConfig) GetHostRoot(alias string) (string, bool) {
	host, ok := c.Hosts[alias]
	if !ok {
		return "", false
	}
	return host.Root, true
}

// DefaultHostAlias returns the default host alias, or empty string if not set.
func (c *GlobalConfig) DefaultHostAlias() string {
	return c.DefaultHost
}

// ResolveWorkspacePath returns the absolute path for a workspace by name.
func (c *GlobalConfig) ResolveWorkspacePath(name string) string {
	return filepath.Join(c.WorkspaceRoot, name)
}

// LoadedConfig wraps GlobalConfig with runtime metadata about where it was loaded from.
type LoadedConfig struct {
	// Config holds the configuration data.
	Config GlobalConfig

	// SourcePath is the file path this config was loaded from (empty if using defaults).
	SourcePath string
}

// ConfigPaths returns the list of config file paths in priority order.
func ConfigPaths() []string {
	home, err := os.UserHomeDir()
	if err != nil {
		return nil
	}

	paths := []string{
		filepath.Join(home, ".wtp.toml"),
		filepath.Join(home, ".wtp", "config.toml"),
	}

	// Platform-specific config directory
	configDir := configDirPath()
	if configDir != "" {
		paths = append(paths, filepath.Join(configDir, "wtp", "config.toml"))
	}

	return paths
}

// LoadConfig loads the configuration from the first existing config file.
// Returns the loaded config and an optional warning about multiple config files.
func LoadConfig() (*LoadedConfig, string, error) {
	paths := ConfigPaths()
	var foundPaths []string
	var loadedPath string
	var cfg *GlobalConfig

	for _, p := range paths {
		if _, err := os.Stat(p); err == nil {
			foundPaths = append(foundPaths, p)
			if cfg == nil {
				loaded, err := loadConfigFromFile(p)
				if err != nil {
					return nil, "", err
				}
				cfg = loaded
				loadedPath = p
			}
		}
	}

	var warning string
	if len(foundPaths) > 1 {
		warning = fmt.Sprintf(
			"Warning: Multiple config files found: %s. Using %s",
			strings.Join(foundPaths, ", "),
			loadedPath,
		)
	}

	if cfg == nil {
		defaultCfg := DefaultGlobalConfig()
		cfg = &defaultCfg
	}

	loaded := &LoadedConfig{
		Config:     *cfg,
		SourcePath: loadedPath,
	}

	return loaded, warning, nil
}

// Save writes the configuration back to its source file,
// or to ~/.wtp/config.toml if no source file exists.
func (lc *LoadedConfig) Save() error {
	configPath := lc.SourcePath
	if configPath == "" {
		home, err := os.UserHomeDir()
		if err != nil {
			return &ConfigError{Message: "could not find home directory"}
		}
		configPath = filepath.Join(home, ".wtp", "config.toml")
	}

	// Create parent directories if needed.
	dir := filepath.Dir(configPath)
	if err := os.MkdirAll(dir, 0755); err != nil {
		return fmt.Errorf("failed to create config directory: %w", err)
	}

	f, err := os.Create(configPath)
	if err != nil {
		return fmt.Errorf("failed to create config file: %w", err)
	}
	defer f.Close()

	encoder := toml.NewEncoder(f)
	if err := encoder.Encode(lc.Config); err != nil {
		return fmt.Errorf("failed to encode config: %w", err)
	}

	return nil
}

// ScanWorkspaces delegates to Config.ScanWorkspaces.
func (lc *LoadedConfig) ScanWorkspaces() map[string]string {
	return lc.Config.ScanWorkspaces()
}

// loadConfigFromFile reads and parses a TOML config file, expanding tilde paths.
func loadConfigFromFile(path string) (*GlobalConfig, error) {
	var cfg GlobalConfig
	if _, err := toml.DecodeFile(path, &cfg); err != nil {
		return nil, fmt.Errorf("failed to parse config file %s: %w", path, err)
	}

	// Expand ~ in workspace_root
	cfg.WorkspaceRoot = ExpandTilde(cfg.WorkspaceRoot)
	if cfg.WorkspaceRoot == "" {
		cfg.WorkspaceRoot = defaultWorkspaceRoot()
	}

	// Expand ~ in host roots
	for name, host := range cfg.Hosts {
		host.Root = ExpandTilde(host.Root)
		cfg.Hosts[name] = host
	}

	// Expand ~ in hook paths
	if cfg.Hooks.OnCreate != "" {
		cfg.Hooks.OnCreate = ExpandTilde(cfg.Hooks.OnCreate)
	}

	return &cfg, nil
}

// ExpandTilde replaces a leading ~ with the user's home directory.
func ExpandTilde(path string) string {
	if !strings.HasPrefix(path, "~") {
		return path
	}
	home, err := os.UserHomeDir()
	if err != nil {
		return path
	}
	if path == "~" {
		return home
	}
	if strings.HasPrefix(path, "~/") || strings.HasPrefix(path, "~\\") {
		return filepath.Join(home, path[2:])
	}
	return path
}

// defaultWorkspaceRoot returns the default workspace root path.
func defaultWorkspaceRoot() string {
	home, err := os.UserHomeDir()
	if err != nil {
		return DefaultWorkspaceRoot
	}
	return filepath.Join(home, DefaultWorkspaceRoot)
}

// configDirPath returns the platform-specific config directory.
// On Linux/macOS: ~/.config
// On Windows: %APPDATA%
func configDirPath() string {
	if runtime.GOOS == "windows" {
		return os.Getenv("APPDATA")
	}
	// XDG_CONFIG_HOME or default ~/.config
	if xdg := os.Getenv("XDG_CONFIG_HOME"); xdg != "" {
		return xdg
	}
	home, err := os.UserHomeDir()
	if err != nil {
		return ""
	}
	return filepath.Join(home, ".config")
}
