package cmd

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/eddix/wtp/internal/core"
	"github.com/fatih/color"
	"github.com/spf13/cobra"
)

// NewSwitchCmd returns the "wtp switch" command.
func NewSwitchCmd() *cobra.Command {
	var (
		create     bool
		branchName string
		baseName   string
	)

	cmd := &cobra.Command{
		Use:     "switch [WORKSPACE]",
		Short:   "Add current repo to a workspace",
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

			// Verify we're in a git repository.
			cwd, err := os.Getwd()
			if err != nil {
				return fmt.Errorf("failed to get current directory: %w", err)
			}

			currentRepoRoot, err := git.GetRepoRoot(cwd)
			if err != nil {
				return fmt.Errorf(
					"current directory is not in a git repository. " +
						"Please run this command from within a git repository",
				)
			}

			cyan := color.New(color.FgCyan)
			dim := color.New(color.Faint)

			fmt.Print("Current repository: ")
			cyan.Println(currentRepoRoot)

			// Determine workspace name.
			var workspaceName string
			if len(args) > 0 {
				workspaceName = args[0]
			} else {
				// No workspace specified - need interactive selection.
				workspaces := manager.ListWorkspaces()
				if len(workspaces) == 0 {
					return fmt.Errorf("no workspaces found. Create one with: wtp create <name>")
				}
				return fmt.Errorf(
					"no workspace specified.\n"+
						"Usage: wtp switch <workspace>\n"+
						"Available workspaces: %s",
					formatWorkspaceNames(workspaces),
				)
			}

			// Get or create target workspace.
			var targetWorkspacePath string
			if wsPath, ok := manager.GlobalConfig().GetWorkspacePath(workspaceName); ok {
				if !pathIsDir(wsPath) {
					if create {
						fmt.Printf("Workspace '%s' exists in config but directory is missing. Recreating...\n",
							workspaceName)
						var err error
						targetWorkspacePath, err = manager.CreateWorkspace(workspaceName, true)
						if err != nil {
							return err
						}
					} else {
						return fmt.Errorf(
							"workspace '%s' directory does not exist at %s. "+
								"Use --create to recreate it",
							workspaceName, wsPath,
						)
					}
				} else {
					targetWorkspacePath = wsPath
				}
			} else {
				// Workspace doesn't exist.
				if create {
					fmt.Printf("Creating new workspace '%s'...\n", workspaceName)
					var err error
					targetWorkspacePath, err = manager.CreateWorkspace(workspaceName, true)
					if err != nil {
						return err
					}
				} else {
					return fmt.Errorf(
						"workspace '%s' does not exist. "+
							"Create it with: wtp create %s\n"+
							"Or use: wtp switch --create %s",
						workspaceName, workspaceName, workspaceName,
					)
				}
			}

			if !pathIsDir(filepath.Join(targetWorkspacePath, core.WtpDir)) {
				return fmt.Errorf(
					"workspace '%s' is missing its .wtp directory. It may be corrupted",
					workspaceName,
				)
			}

			fmt.Print("Target workspace: ")
			cyan.Print(workspaceName)
			fmt.Print(" at ")
			dim.Println(targetWorkspacePath)

			// Fence check.
			fence := core.NewFenceFromConfig(manager.GlobalConfig())
			if !fence.IsWithinBoundary(targetWorkspacePath) {
				fmt.Fprintf(os.Stderr,
					"Warning: Workspace '%s' is outside workspace_root: %s\n"+
						"Target path: %s\n",
					workspaceName, fence.Boundary(), targetWorkspacePath,
				)
			}

			// Try to match repository to a host alias.
			var repoRef core.RepoRef
			if ref := manager.MatchHostAlias(currentRepoRoot); ref != nil {
				fmt.Print("Matched to host alias: ")
				cyan.Print(ref.Host)
				fmt.Print(" (")
				dim.Print(ref.Path)
				fmt.Println(")")
				repoRef = *ref
			} else {
				fmt.Println("Using absolute path (no matching host alias found)")
				repoRef = core.NewAbsoluteRepoRef(currentRepoRoot)
			}

			// Determine branch name.
			branch := branchName
			if branch == "" {
				branch = workspaceName
			}

			// Determine base reference.
			base := baseName
			if base == "" {
				currentBranch, err := git.GetCurrentBranch(currentRepoRoot)
				if err != nil {
					base = "HEAD"
				} else {
					base = currentBranch
				}
			}

			// Load existing worktrees.
			wtManager, err := core.LoadWorktreeManager(targetWorkspacePath, nil)
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
			worktreePathAbs := filepath.Join(targetWorkspacePath, worktreePathRel)

			fmt.Print("Creating worktree at: ")
			cyan.Println(worktreePathAbs)

			if pathExists(worktreePathAbs) {
				return fmt.Errorf("worktree directory already exists at %s", worktreePathAbs)
			}

			// Create the worktree.
			branchExists, err := git.BranchExists(currentRepoRoot, branch)
			if err != nil {
				return err
			}

			if branchExists {
				fmt.Print("Using existing branch: ")
				cyan.Println(branch)
				if err := git.AddWorktreeForBranch(currentRepoRoot, worktreePathAbs, branch); err != nil {
					return err
				}
			} else {
				fmt.Printf("Creating new branch '")
				cyan.Print(branch)
				fmt.Print("' from ")
				dim.Println(base)
				if err := git.CreateWorktreeWithBranch(currentRepoRoot, worktreePathAbs, branch, base); err != nil {
					return err
				}
			}

			// Get HEAD commit.
			headCommit, _ := git.GetHeadCommit(worktreePathAbs, false)

			// Record in worktree.toml.
			wtManager2, err := core.LoadWorktreeManager(targetWorkspacePath, nil)
			if err != nil {
				return err
			}
			entry := core.NewWorktreeEntry(repoRef, branch, worktreePathRel, base, headCommit)
			if err := wtManager2.AddWorktree(entry); err != nil {
				return err
			}

			repoName := filepath.Base(currentRepoRoot)

			green := color.New(color.FgGreen, color.Bold)
			green.Print("\u2713 ")
			fmt.Printf("Successfully switched '")
			cyan.Print(repoName)
			fmt.Printf("' to workspace '")
			cyan.Print(workspaceName)
			fmt.Println("'")
			fmt.Println()
			fmt.Printf("Worktree created at: ")
			cyan.Println(worktreePathAbs)
			fmt.Println()
			fmt.Println("To start working:")
			fmt.Print("  ")
			cyan.Printf("cd %s\n", worktreePathAbs)

			return nil
		},
	}

	cmd.Flags().BoolVarP(&create, "create", "c", false, "Create the workspace if it doesn't exist")
	cmd.Flags().StringVarP(&branchName, "branch", "b", "", "Branch name to use (defaults to workspace name)")
	cmd.Flags().StringVarP(&baseName, "base", "B", "", "Base reference to create branch from")

	return cmd
}

// formatWorkspaceNames returns a comma-separated list of workspace names.
func formatWorkspaceNames(workspaces []core.WorkspaceInfo) string {
	names := make([]string, len(workspaces))
	for i, ws := range workspaces {
		names[i] = ws.Name
	}
	result := ""
	for i, n := range names {
		if i > 0 {
			result += ", "
		}
		result += n
	}
	return result
}

func init() {
	rootCmd.AddCommand(NewSwitchCmd())
}
