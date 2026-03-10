package cmd

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/eddix/wtp/internal/core"
	"github.com/fatih/color"
	"github.com/spf13/cobra"
)

// NewEjectCmd returns the "wtp eject" command.
func NewEjectCmd() *cobra.Command {
	var force bool

	cmd := &cobra.Command{
		Use:     "eject [REPO]",
		Short:   "Remove a repository worktree from the current workspace",
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

			// Load worktrees.
			wtManager, err := core.LoadWorktreeManager(workspacePath, nil)
			if err != nil {
				return err
			}

			worktrees := wtManager.ListWorktrees()
			if len(worktrees) == 0 {
				return fmt.Errorf("no worktrees in this workspace")
			}

			// Determine which repo to eject.
			var slug string
			if len(args) > 0 {
				slug = args[0]
			} else {
				// No repo specified - list available.
				items := make([]string, len(worktrees))
				for i, wt := range worktrees {
					items[i] = fmt.Sprintf("%s (%s, branch: %s)",
						wt.Repo.Slug(), wt.Repo.Display(), wt.Branch)
				}
				return fmt.Errorf(
					"no repository specified.\n"+
						"Usage: wtp eject <repo>\n"+
						"Available worktrees:\n  %s",
					joinLines(items),
				)
			}

			// Find the worktree entry.
			entry := wtManager.Config().FindBySlug(slug)
			if entry == nil {
				available := make([]string, len(worktrees))
				for i, wt := range worktrees {
					available[i] = wt.Repo.Slug()
				}
				return fmt.Errorf(
					"worktree '%s' not found in workspace.\nAvailable: %s",
					slug, joinComma(available),
				)
			}

			repoDisplay := entry.Repo.Display()
			branch := entry.Branch
			worktreePathRel := entry.WorktreePath
			worktreePathAbs := filepath.Join(workspacePath, worktreePathRel)
			removalSlug := entry.Repo.Slug()

			cyan := color.New(color.FgCyan)
			bold := color.New(color.Bold)
			dim := color.New(color.Faint)

			fmt.Print("Ejecting from workspace: ")
			cyan.Println(workspaceName)
			fmt.Println()
			fmt.Print("  ")
			bold.Print("Repository:   ")
			cyan.Println(repoDisplay)
			fmt.Print("  ")
			bold.Print("Branch:       ")
			cyan.Println(branch)
			fmt.Print("  ")
			bold.Print("Worktree:     ")
			dim.Println(worktreePathAbs)
			fmt.Println()

			if pathExists(worktreePathAbs) {
				// Safety check: is the worktree dirty?
				status, err := git.GetStatus(worktreePathAbs)
				if err != nil {
					return err
				}

				if status.Dirty && !force {
					return fmt.Errorf(
						"worktree has uncommitted changes:\n  %s\n\n"+
							"Commit or stash your changes first, or use --force to eject anyway",
						status.FormatDetailStatus(),
					)
				}
				if status.Dirty && force {
					fmt.Fprintf(os.Stderr,
						"Warning: Worktree has uncommitted changes (%s), proceeding with --force.\n",
						status.FormatDetailStatus(),
					)
				}

				// Resolve the repo root from the worktree path.
				repoRoot, err := git.GetRepoRoot(worktreePathAbs)
				if err != nil {
					return err
				}
				if err := git.RemoveWorktree(repoRoot, worktreePathAbs, force); err != nil {
					return err
				}
			} else {
				fmt.Fprintf(os.Stderr,
					"Note: Worktree directory not found at %s, cleaning up record only.\n",
					worktreePathAbs,
				)
			}

			// Remove from worktree.toml.
			wtManager2, err := core.LoadWorktreeManager(workspacePath, nil)
			if err != nil {
				return err
			}
			if _, err := wtManager2.RemoveWorktree(removalSlug); err != nil {
				return err
			}

			green := color.New(color.FgGreen, color.Bold)
			green.Print("\u2713 ")
			fmt.Println("Worktree ejected successfully.")

			return nil
		},
	}

	cmd.Flags().BoolVarP(&force, "force", "f", false, "Force eject even if worktree has uncommitted changes")

	return cmd
}

// joinLines joins strings with newline + indent.
func joinLines(items []string) string {
	result := ""
	for i, item := range items {
		if i > 0 {
			result += "\n  "
		}
		result += item
	}
	return result
}

// joinComma joins strings with comma and space.
func joinComma(items []string) string {
	result := ""
	for i, item := range items {
		if i > 0 {
			result += ", "
		}
		result += item
	}
	return result
}

func init() {
	rootCmd.AddCommand(NewEjectCmd())
}
