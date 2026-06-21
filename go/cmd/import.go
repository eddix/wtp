package cmd

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/eddix/wtp/internal/core"
	"github.com/fatih/color"
	"github.com/spf13/cobra"
)

// NewImportCmd returns the "wtp import" command.
func NewImportCmd() *cobra.Command {
	var (
		hostAlias  string
		repoFlag   string
		branchName string
		baseName   string
	)

	cmd := &cobra.Command{
		Use:     "import [PATH]",
		Short:   "Import a repository worktree into the current workspace",
		GroupID: GroupRepository,
		Args:    cobra.MaximumNArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			git := core.NewGitClient()
			if err := git.CheckGit(); err != nil {
				return err
			}

			loadedConfig, warning, err := core.LoadConfig()
			if err != nil {
				return err
			}
			if warning != "" {
				color.Yellow("%s", warning)
			}

			core.InitGlobalFence(loadedConfig.Config.WorkspaceRoot)

			manager := core.NewWorkspaceManager(loadedConfig)

			// Detect current workspace.
			cwd, err := os.Getwd()
			if err != nil {
				return fmt.Errorf("failed to get current directory: %w", err)
			}

			workspaceName, workspacePath, err := manager.DetectCurrentWorkspace(cwd)
			if err != nil {
				return err
			}

			if !pathIsDir(workspacePath) {
				return fmt.Errorf(
					"workspace '%s' directory does not exist at %s",
					workspaceName, workspacePath,
				)
			}
			if !pathIsDir(filepath.Join(workspacePath, core.WtpDir)) {
				return fmt.Errorf(
					"workspace '%s' exists in config but the directory is missing or corrupted",
					workspaceName,
				)
			}

			cyan := color.New(color.FgCyan)
			dim := color.New(color.Faint)

			fmt.Print("Importing into workspace: ")
			cyan.Print(workspaceName)
			fmt.Print(" at ")
			dim.Println(workspacePath)

			// Fence check.
			fence := core.NewFenceFromConfig(manager.GlobalConfig())
			if !fence.IsWithinBoundary(workspacePath) {
				fmt.Fprintf(os.Stderr,
					"Warning: Workspace '%s' is outside workspace_root: %s\n"+
						"Target path: %s\n",
					workspaceName, fence.Boundary(), workspacePath,
				)
			}

			// Resolve repository reference.
			var repoRef core.RepoRef
			if repoFlag != "" {
				// --repo flag provided.
				if len(args) > 0 {
					return fmt.Errorf("cannot specify both <path> argument and --repo")
				}
				expanded := core.ExpandTilde(repoFlag)
				if _, err := os.Stat(expanded); os.IsNotExist(err) {
					return fmt.Errorf("repository not found: %s", expanded)
				}
				repoRef = core.NewAbsoluteRepoRef(expanded)
			} else if len(args) > 0 {
				// Positional path argument.
				repoRef, err = resolveRepoRef(manager, args[0], hostAlias)
				if err != nil {
					return err
				}
			} else {
				// No path or repo specified - need interactive selection.
				return fmt.Errorf(
					"no repository specified.\n" +
						"Usage: wtp import <path>\n" +
						"  or:  wtp import --repo <path>\n" +
						"  or:  wtp import -H <host> <path>",
				)
			}

			// Get absolute path to repository.
			hosts := buildHostMap(manager)
			repoPath := repoRef.ToAbsolutePath(hosts)

			// Verify it's a git repository.
			if !git.IsInGitRepo(repoPath) {
				return fmt.Errorf("%s is not a git repository", repoPath)
			}

			repoRoot, err := git.GetRepoRoot(repoPath)
			if err != nil {
				return err
			}
			isBare, _ := git.IsBareRepo(repoRoot)

			fmt.Print("Repository: ")
			cyan.Print(repoRef.Display())
			fmt.Print(" at ")
			dim.Print(repoRoot)
			if isBare {
				dim.Print(" (bare)")
			}
			fmt.Println()

			// Determine branch name.
			branch := branchName
			if branch == "" {
				branch = workspaceName
			}

			// Determine base reference.
			base := baseName
			if base == "" {
				currentBranch, err := git.GetCurrentBranch(repoRoot)
				if err != nil {
					base = "HEAD"
				} else {
					base = currentBranch
				}
			}

			// Load existing worktrees.
			wtManager, err := core.LoadWorktreeManager(workspacePath, nil)
			if err != nil {
				return err
			}

			// Check for duplicate repo.
			if existing := wtManager.Config().FindByRepo(repoRef); existing != nil {
				return fmt.Errorf(
					"repository '%s' is already in this workspace with branch '%s'.\n"+
						"Each repository can only have one worktree per workspace.\n"+
						"Existing worktree: %s",
					repoRef.Display(), existing.Branch, existing.WorktreePath,
				)
			}

			// Generate worktree path.
			repoSlug := repoRef.Slug()
			worktreePathRel := wtManager.GenerateWorktreePath(repoSlug)
			worktreePathAbs := filepath.Join(workspacePath, worktreePathRel)

			fmt.Print("Creating worktree at: ")
			cyan.Println(worktreePathAbs)

			if pathExists(worktreePathAbs) {
				return fmt.Errorf("worktree directory already exists at %s", worktreePathAbs)
			}

			// Create the worktree.
			branchExists, err := git.BranchExists(repoRoot, branch)
			if err != nil {
				return err
			}

			if branchExists {
				fmt.Print("Using existing branch: ")
				cyan.Println(branch)
				if err := git.AddWorktreeForBranch(repoRoot, worktreePathAbs, branch); err != nil {
					return err
				}
			} else {
				fmt.Printf("Creating new branch '")
				cyan.Print(branch)
				fmt.Print("' from ")
				dim.Println(base)
				if err := git.CreateWorktreeWithBranch(repoRoot, worktreePathAbs, branch, base); err != nil {
					return err
				}
			}

			// Get HEAD commit.
			headCommit, _ := git.GetHeadCommit(worktreePathAbs, false)

			// Record in worktree.toml.
			wtManager2, err := core.LoadWorktreeManager(workspacePath, nil)
			if err != nil {
				return err
			}
			entry := core.NewWorktreeEntry(repoRef, branch, worktreePathRel, base, headCommit)
			if err := wtManager2.AddWorktree(entry); err != nil {
				return err
			}

			green := color.New(color.FgGreen, color.Bold)
			green.Print("\u2713 ")
			fmt.Println("Worktree imported successfully!")

			return nil
		},
	}

	cmd.Flags().StringVarP(&hostAlias, "host", "H", "", "Host alias to use for resolving the repository path")
	cmd.Flags().StringVarP(&repoFlag, "repo", "r", "", "Full repository path (alternative to PATH)")
	cmd.Flags().StringVarP(&branchName, "branch", "b", "", "Branch name to use (defaults to workspace name)")
	cmd.Flags().StringVarP(&baseName, "base", "B", "", "Base reference to create branch from")

	return cmd
}

// resolveRepoRef resolves a repository reference from path and optional host.
func resolveRepoRef(manager *core.WorkspaceManager, path, host string) (core.RepoRef, error) {
	if host != "" {
		// Explicit host specified.
		_, ok := manager.GlobalConfig().GetHostRoot(host)
		if !ok {
			return core.RepoRef{}, fmt.Errorf("host alias '%s' not found in config", host)
		}
		return core.NewHostedRepoRef(host, path), nil
	}

	if alias := manager.GlobalConfig().DefaultHostAlias(); alias != "" {
		return core.NewHostedRepoRef(alias, path), nil
	}

	// Treat as absolute/relative path.
	expanded := core.ExpandTilde(path)
	absPath := expanded
	if !filepath.IsAbs(expanded) {
		cwd, err := os.Getwd()
		if err != nil {
			return core.RepoRef{}, err
		}
		absPath = filepath.Join(cwd, expanded)
	}

	return core.NewAbsoluteRepoRef(absPath), nil
}

// buildHostMap builds a map from host alias to root path string.
func buildHostMap(manager *core.WorkspaceManager) map[string]string {
	hosts := manager.GetHosts()
	result := make(map[string]string, len(hosts))
	for alias, cfg := range hosts {
		result[alias] = cfg.Root
	}
	return result
}

func init() {
	rootCmd.AddCommand(NewImportCmd())
}
