package cmd

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/eddix/wtp/internal/core"
	"github.com/fatih/color"
	"github.com/spf13/cobra"
)

// NewStatusCmd returns the "wtp status" command.
func NewStatusCmd() *cobra.Command {
	var workspace string
	var long bool

	cmd := &cobra.Command{
		Use:     "status",
		Short:   "Show worktree status for a workspace",
		GroupID: GroupWorkspace,
		Args:    cobra.NoArgs,
		RunE: func(cmd *cobra.Command, args []string) error {
			loadedConfig, warning, err := core.LoadConfig()
			if err != nil {
				return err
			}
			if warning != "" {
				color.Yellow("%s", warning)
			}

			manager := core.NewWorkspaceManager(loadedConfig)

			// Determine target workspace.
			var workspaceName, workspacePath string
			if workspace != "" {
				workspaceName = workspace
				path, ok := manager.GlobalConfig().GetWorkspacePath(workspace)
				if !ok {
					return fmt.Errorf(
						"workspace '%s' not found. Create it with: wtp create %s",
						workspace, workspace,
					)
				}
				workspacePath = path
			} else {
				cwd, err := os.Getwd()
				if err != nil {
					return fmt.Errorf("failed to get current directory: %w", err)
				}
				name, path, err := manager.DetectCurrentWorkspace(cwd)
				if err != nil {
					return err
				}
				workspaceName = name
				workspacePath = path
			}

			if info, err := os.Stat(workspacePath); err != nil || !info.IsDir() {
				return fmt.Errorf(
					"workspace '%s' directory does not exist at %s",
					workspaceName, workspacePath,
				)
			}

			wtpDir := filepath.Join(workspacePath, core.WtpDir)
			if info, err := os.Stat(wtpDir); err != nil || !info.IsDir() {
				return fmt.Errorf(
					"workspace '%s' exists in config but the directory is missing or corrupted",
					workspaceName,
				)
			}

			cyan := color.New(color.FgCyan, color.Bold)
			dim := color.New(color.Faint)

			fmt.Print("Workspace: ")
			cyan.Print(workspaceName)
			fmt.Print(" at ")
			dim.Println(workspacePath)
			fmt.Println()

			// Load worktrees.
			wtMgr, err := core.LoadWorktreeManager(workspacePath, nil)
			if err != nil {
				return err
			}
			worktrees := wtMgr.ListWorktrees()

			if len(worktrees) == 0 {
				dim.Println("No worktrees in this workspace.")
				fmt.Println()
				fmt.Println("Import a worktree with:")
				fmt.Print("  ")
				cyan.Println("wtp import <repo_path>")
				fmt.Println()
				fmt.Println("Or switch the current repo to this workspace:")
				fmt.Print("  ")
				cyan.Printf("wtp switch %s\n", workspaceName)
				return nil
			}

			git := core.NewGitClient()

			if long {
				return printDetailedStatus(git, worktrees, workspacePath)
			}
			return printCompactStatus(git, worktrees, workspacePath)
		},
	}

	cmd.Flags().StringVarP(&workspace, "workspace", "w", "", "Workspace to show status for")
	cmd.Flags().BoolVarP(&long, "long", "l", false, "Show detailed information")

	return cmd
}

func printCompactStatus(git *core.GitClient, worktrees []core.WorktreeEntry, workspacePath string) error {
	bold := color.New(color.Bold)
	cyan := color.New(color.FgCyan)
	red := color.New(color.FgRed, color.Bold)

	bold.Printf("%-30s %-20s %s\n", "REPOSITORY", "BRANCH", "STATUS")

	for _, wt := range worktrees {
		wtFullPath := filepath.Join(workspacePath, wt.WorktreePath)
		repoDisplay := wt.Repo.Display()

		if info, err := os.Stat(wtFullPath); err != nil || !info.IsDir() {
			// Truncate long names.
			if len(repoDisplay) > 30 {
				repoDisplay = repoDisplay[:27] + "..."
			}
			fmt.Printf("%-30s ", repoDisplay)
			cyan.Printf("%-20s ", wt.Branch)
			red.Println("missing")
			continue
		}

		statusStr := "?"
		if status, err := git.GetStatus(wtFullPath); err == nil {
			statusStr = status.FormatCompact()
		}

		// Truncate long names.
		if len(repoDisplay) > 30 {
			repoDisplay = repoDisplay[:27] + "..."
		}

		fmt.Printf("%-30s ", repoDisplay)
		cyan.Printf("%-20s ", wt.Branch)
		fmt.Println(statusStr)
	}

	return nil
}

func printDetailedStatus(git *core.GitClient, worktrees []core.WorktreeEntry, workspacePath string) error {
	bold := color.New(color.Bold)
	cyan := color.New(color.FgCyan, color.Bold)
	yellow := color.New(color.FgYellow)
	red := color.New(color.FgRed, color.Bold)
	dim := color.New(color.Faint)

	separator := "\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500" +
		"\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500" +
		"\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500" +
		"\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500" +
		"\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500" +
		"\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500"

	for _, wt := range worktrees {
		wtFullPath := filepath.Join(workspacePath, wt.WorktreePath)
		repoDisplay := wt.Repo.Display()

		dim.Println(separator)
		fmt.Print("  ")
		cyan.Println(repoDisplay)
		dim.Println(separator)

		if info, err := os.Stat(wtFullPath); err != nil || !info.IsDir() {
			fmt.Print("  ")
			bold.Print("Status:   ")
			red.Println("MISSING")
			fmt.Println()
			continue
		}

		// Branch.
		fmt.Print("  ")
		bold.Print("Branch:   ")
		color.New(color.FgCyan).Println(wt.Branch)

		// HEAD: hash + subject + relative time.
		headShort, _ := git.GetHeadCommit(wtFullPath, true)
		subject, _ := git.GetLastCommitSubject(wtFullPath)
		relTime, _ := git.GetLastCommitRelativeTime(wtFullPath)

		if headShort != "" {
			fmt.Print("  ")
			bold.Print("HEAD:     ")
			yellow.Print(headShort)
			fmt.Printf(" %s ", subject)
			dim.Printf("(%s)\n", relTime)
		}

		// Status.
		status, err := git.GetStatus(wtFullPath)
		if err != nil {
			fmt.Print("  ")
			bold.Print("Status:   ")
			red.Printf("error: %v\n", err)
		} else {
			fmt.Print("  ")
			bold.Print("Status:   ")
			fmt.Println(status.FormatDetailStatus())

			fmt.Print("  ")
			bold.Print("Remote:   ")
			fmt.Println(status.FormatDetailRemote())
		}

		// Stash.
		stashCount, err := git.GetStashCount(wtFullPath)
		if err == nil {
			fmt.Print("  ")
			bold.Print("Stash:    ")
			if stashCount > 0 {
				entryWord := "entries"
				if stashCount == 1 {
					entryWord = "entry"
				}
				yellow.Printf("%d %s\n", stashCount, entryWord)
			} else {
				dim.Println("none")
			}
		}

		fmt.Println()
	}

	dim.Println(separator)
	return nil
}

func init() {
	rootCmd.AddCommand(NewStatusCmd())
}
