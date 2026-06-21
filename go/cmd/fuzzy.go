package cmd

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/eddix/wtp/internal/core"
	"github.com/ktr0731/go-fuzzyfinder"
)

// isInteractive checks if stdin and stderr are connected to a TTY.
func isInteractive() bool {
	stdinInfo, err := os.Stdin.Stat()
	if err != nil {
		return false
	}
	stderrInfo, err := os.Stderr.Stat()
	if err != nil {
		return false
	}
	return (stdinInfo.Mode()&os.ModeCharDevice) != 0 &&
		(stderrInfo.Mode()&os.ModeCharDevice) != 0
}

// resolveWorkspaceInteractively prompts the user to select a workspace
// using fuzzy finder when no workspace argument was provided.
func resolveWorkspaceInteractively(manager *core.WorkspaceManager, command string) (string, error) {
	workspaces := manager.ListWorkspaces()

	if len(workspaces) == 0 {
		return "", fmt.Errorf("no workspaces found. Create one with: wtp create <name>")
	}

	if !isInteractive() {
		return "", fmt.Errorf(
			"no workspace specified and not running in an interactive terminal.\nUsage: %s <workspace>",
			command,
		)
	}

	idx, err := fuzzyfinder.Find(workspaces, func(i int) string {
		return fmt.Sprintf("%s    (%s)", workspaces[i].Name, workspaces[i].Path)
	}, fuzzyfinder.WithPromptString(command+" > "))
	if err != nil {
		return "", fmt.Errorf("selection cancelled")
	}

	return workspaces[idx].Name, nil
}

// resolveHostInteractively prompts the user to select a host alias.
// If only one host is configured, returns it directly.
func resolveHostInteractively(manager *core.WorkspaceManager, command string) (string, error) {
	hosts := manager.GetHosts()

	if len(hosts) == 0 {
		return "", fmt.Errorf("no hosts configured. Add one with: wtp host add <alias> <path>")
	}

	// Single host: return directly.
	if len(hosts) == 1 {
		for alias := range hosts {
			return alias, nil
		}
	}

	if !isInteractive() {
		return "", fmt.Errorf(
			"no host specified and not running in an interactive terminal.\nUsage: %s -H <host>",
			command,
		)
	}

	// Build sorted list for display.
	type hostItem struct {
		Alias string
		Root  string
	}
	items := make([]hostItem, 0, len(hosts))
	for alias, cfg := range hosts {
		items = append(items, hostItem{Alias: alias, Root: cfg.Root})
	}
	sort.Slice(items, func(i, j int) bool { return items[i].Alias < items[j].Alias })

	idx, err := fuzzyfinder.Find(items, func(i int) string {
		return fmt.Sprintf("%s    (%s)", items[i].Alias, items[i].Root)
	}, fuzzyfinder.WithPromptString(command+" (select host) > "))
	if err != nil {
		return "", fmt.Errorf("selection cancelled")
	}

	return items[idx].Alias, nil
}

// isBareGitRepo checks if a directory looks like a bare git repository.
func isBareGitRepo(path string) bool {
	_, errGit := os.Stat(filepath.Join(path, ".git"))
	infoHead, errHead := os.Stat(filepath.Join(path, "HEAD"))
	infoObjects, errObjects := os.Stat(filepath.Join(path, "objects"))
	infoRefs, errRefs := os.Stat(filepath.Join(path, "refs"))

	return errGit != nil &&
		errHead == nil && !infoHead.IsDir() &&
		errObjects == nil && infoObjects.IsDir() &&
		errRefs == nil && infoRefs.IsDir()
}

// scanGitRepos scans a directory for git repositories (normal and bare).
// Returns paths relative to root. Limits depth to 4 levels.
func scanGitRepos(root string) []string {
	var repos []string
	var skipPrefixes []string

	scanDir(root, root, 0, 4, &repos, &skipPrefixes)

	sort.Strings(repos)
	return repos
}

// scanDir recursively scans for git repos up to maxDepth.
func scanDir(root, current string, depth, maxDepth int, repos *[]string, skipPrefixes *[]string) {
	if depth > maxDepth {
		return
	}

	entries, err := os.ReadDir(current)
	if err != nil {
		return
	}

	for _, entry := range entries {
		if !entry.IsDir() {
			continue
		}

		name := entry.Name()

		// Skip hidden directories, but allow .git-suffixed dirs (bare repos).
		if depth > 0 && strings.HasPrefix(name, ".") && !strings.HasSuffix(name, ".git") {
			continue
		}

		path := filepath.Join(current, name)

		// Skip if inside a previously found repo.
		skip := false
		for _, prefix := range *skipPrefixes {
			if strings.HasPrefix(path, prefix+string(filepath.Separator)) || path == prefix {
				skip = true
				break
			}
		}
		if skip {
			continue
		}

		// Check for normal repo (.git exists) or bare repo.
		_, errDotGit := os.Stat(filepath.Join(path, ".git"))
		isRepo := errDotGit == nil || isBareGitRepo(path)

		if isRepo {
			rel, err := filepath.Rel(root, path)
			if err == nil && rel != "" && rel != "." {
				*repos = append(*repos, filepath.ToSlash(rel))
				*skipPrefixes = append(*skipPrefixes, path)
				continue // Don't recurse into this repo.
			}
		}

		// Recurse into subdirectories.
		scanDir(root, path, depth+1, maxDepth, repos, skipPrefixes)
	}
}

// resolveRepoInteractively scans host root for git repos and lets user pick one.
func resolveRepoInteractively(manager *core.WorkspaceManager, hostAlias, command string) (string, error) {
	hostRoot, ok := manager.GlobalConfig().GetHostRoot(hostAlias)
	if !ok {
		return "", fmt.Errorf("host alias '%s' not found in config", hostAlias)
	}

	repos := scanGitRepos(hostRoot)

	if len(repos) == 0 {
		return "", fmt.Errorf("no git repositories found under host '%s' (%s)", hostAlias, hostRoot)
	}

	if !isInteractive() {
		return "", fmt.Errorf(
			"no repository specified and not running in an interactive terminal.\nUsage: %s <path>",
			command,
		)
	}

	idx, err := fuzzyfinder.Find(repos, func(i int) string {
		return repos[i]
	}, fuzzyfinder.WithPromptString(command+" (select repo) > "))
	if err != nil {
		return "", fmt.Errorf("selection cancelled")
	}

	return repos[idx], nil
}

// selectWorktreeInteractively prompts the user to select a worktree from the list.
func selectWorktreeInteractively(worktrees []core.WorktreeEntry, command string) (*core.WorktreeEntry, error) {
	if len(worktrees) == 0 {
		return nil, fmt.Errorf("no worktrees in this workspace")
	}

	if !isInteractive() {
		return nil, fmt.Errorf(
			"no worktree specified and not running in an interactive terminal.\nUsage: %s <repo>",
			command,
		)
	}

	idx, err := fuzzyfinder.Find(worktrees, func(i int) string {
		return fmt.Sprintf("%s  [%s]", worktrees[i].Repo.Display(), worktrees[i].Branch)
	}, fuzzyfinder.WithPromptString(command+" (select worktree) > "))
	if err != nil {
		return nil, fmt.Errorf("selection cancelled")
	}

	return &worktrees[idx], nil
}
