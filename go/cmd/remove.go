package cmd

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/eddix/wtp/internal/core"
	"github.com/fatih/color"
	"github.com/spf13/cobra"
)

// NewRemoveCmd returns the "wtp rm" command (with "remove" as alias).
func NewRemoveCmd() *cobra.Command {
	var force bool

	cmd := &cobra.Command{
		Use:     "rm <NAME>",
		Aliases: []string{"remove"},
		Short:   "Remove a workspace",
		GroupID: GroupWorkspace,
		Args:    cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			name := args[0]

			loadedConfig, warning, err := core.LoadConfig()
			if err != nil {
				return err
			}
			if warning != "" {
				color.Yellow("%s", warning)
			}

			core.InitGlobalFence(loadedConfig.Config.WorkspaceRoot)

			manager := core.NewWorkspaceManager(loadedConfig)
			git := core.NewGitClient()
			if err := git.CheckGit(); err != nil {
				return err
			}

			// Check if workspace exists.
			workspacePath, ok := manager.GlobalConfig().GetWorkspacePath(name)
			if !ok {
				return fmt.Errorf("workspace '%s' not found", name)
			}

			cyan := color.New(color.FgCyan)
			green := color.New(color.FgGreen, color.Bold)
			bold := color.New(color.Bold)
			red := color.New(color.FgRed, color.Bold)
			yellowBold := color.New(color.FgYellow, color.Bold)

			fmt.Printf("Removing workspace: ")
			cyan.Println(name)
			fmt.Println()

			// Phase 1: Eject all worktrees.
			wtManager, err := core.LoadWorktreeManager(workspacePath, nil)
			if err != nil {
				return err
			}
			worktrees := wtManager.ListWorktrees()

			if len(worktrees) > 0 {
				bold.Println("Ejecting worktrees:")

				// Pre-check: if not --force, check for dirty worktrees first.
				if !force {
					var dirtyRepos []struct {
						display string
						detail  string
					}
					for _, entry := range worktrees {
						wtPath := filepath.Join(workspacePath, entry.WorktreePath)
						if pathExists(wtPath) {
							if status, err := git.GetStatus(wtPath); err == nil && status.Dirty {
								dirtyRepos = append(dirtyRepos, struct {
									display string
									detail  string
								}{entry.Repo.Display(), status.FormatDetailStatus()})
							}
						}
					}
					if len(dirtyRepos) > 0 {
						fmt.Fprintln(os.Stderr)
						red.Fprintf(os.Stderr, "Error:")
						fmt.Fprintln(os.Stderr, " The following worktrees have uncommitted changes:")
						fmt.Fprintln(os.Stderr)
						for _, dr := range dirtyRepos {
							fmt.Fprintf(os.Stderr, "  %s  (%s)\n", color.CyanString("%s", dr.display), dr.detail)
						}
						fmt.Fprintln(os.Stderr)
						fmt.Fprintf(os.Stderr, "Commit or stash your changes first, or use %s to remove anyway.\n", bold.Sprint("--force"))
						os.Exit(1)
					}
				}

				for _, entry := range worktrees {
					wtPath := filepath.Join(workspacePath, entry.WorktreePath)
					slug := entry.Repo.Slug()

					if pathExists(wtPath) {
						if status, err := git.GetStatus(wtPath); err == nil && status.Dirty {
							fmt.Fprintf(os.Stderr, "  ")
							yellowBold.Fprintf(os.Stderr, "Warning:")
							fmt.Fprintf(os.Stderr, " %s (%s), proceeding with --force.\n",
								color.CyanString("%s", slug), status.FormatDetailStatus())
						}

						repoRoot, err := git.GetRepoRoot(wtPath)
						if err != nil {
							fmt.Fprintf(os.Stderr, "  ")
							yellowBold.Fprintf(os.Stderr, "!")
							fmt.Fprintf(os.Stderr, " %s — could not resolve repo root, cleaning up record only.\n",
								color.CyanString("%s", slug))
							continue
						}

						if err := git.RemoveWorktree(repoRoot, wtPath, force); err != nil {
							fmt.Fprintf(os.Stderr, "  ")
							red.Fprintf(os.Stderr, "\u2717")
							fmt.Fprintf(os.Stderr, " %s — %v\n", color.CyanString("%s", slug), err)
							if !force {
								return fmt.Errorf("failed to eject '%s'. Use %s to force removal",
									slug, bold.Sprint("--force"))
							}
						} else {
							fmt.Print("  ")
							green.Print("\u2713")
							fmt.Printf(" %s\n", color.CyanString("%s", slug))
						}
					} else {
						fmt.Fprintf(os.Stderr, "  ")
						yellowBold.Fprintf(os.Stderr, "!")
						fmt.Fprintf(os.Stderr, " %s — directory not found, cleaning up record only.\n",
							color.CyanString("%s", slug))
					}
				}

				// Clear all entries from worktree.toml.
				wtManager2, err := core.LoadWorktreeManager(workspacePath, nil)
				if err == nil {
					for _, entry := range worktrees {
						wtManager2.RemoveWorktree(entry.Repo.Slug())
					}
				}

				fmt.Println()
			}

			// Phase 2: Check remaining contents and remove workspace directory.
			remaining := listRemainingContents(workspacePath)

			if len(remaining) == 0 {
				// Only .wtp directory left — safe to remove.
				if _, err := manager.RemoveWorkspace(name, true); err != nil {
					return err
				}
				green.Print("\u2713 ")
				fmt.Printf("Workspace '")
				cyan.Print(name)
				fmt.Println("' removed.")
			} else {
				// Extra files/dirs exist.
				yellowBold.Fprintf(os.Stderr, "Note:")
				fmt.Fprintln(os.Stderr, " Workspace directory has extra files besides worktrees:")
				fmt.Fprintln(os.Stderr)
				dim := color.New(color.Faint)
				for _, item := range remaining {
					fmt.Fprint(os.Stderr, "  ")
					dim.Fprintln(os.Stderr, item)
				}
				fmt.Fprintln(os.Stderr)

				if force {
					if _, err := manager.RemoveWorkspace(name, true); err != nil {
						return err
					}
					green.Print("\u2713 ")
					fmt.Printf("Workspace '")
					cyan.Print(name)
					fmt.Println("' removed (including extra files).")
				} else {
					fmt.Fprintf(os.Stderr, "Use %s to remove anyway, or clean up these files first.\n",
						bold.Sprint("--force"))
					os.Exit(1)
				}
			}

			return nil
		},
	}

	cmd.Flags().BoolVarP(&force, "force", "f", false, "Force removal even if worktrees have uncommitted changes")

	return cmd
}

// listRemainingContents returns non-.wtp items in the workspace directory.
func listRemainingContents(workspacePath string) []string {
	var remaining []string
	entries, err := os.ReadDir(workspacePath)
	if err != nil {
		return remaining
	}
	for _, entry := range entries {
		if entry.Name() == ".wtp" {
			continue
		}
		remaining = append(remaining, entry.Name())
	}
	return remaining
}

func init() {
	rootCmd.AddCommand(NewRemoveCmd())
}
