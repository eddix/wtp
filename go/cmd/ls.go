package cmd

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/eddix/wtp/internal/core"
	"github.com/fatih/color"
	"github.com/spf13/cobra"
)

// NewLsCmd returns the "wtp ls" command.
func NewLsCmd() *cobra.Command {
	var long bool
	var short bool

	cmd := &cobra.Command{
		Use:     "ls",
		Short:   "List all workspaces",
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
			workspaces := manager.ListWorkspaces()

			if len(workspaces) == 0 {
				if !short {
					dim := color.New(color.Faint)
					cyan := color.New(color.FgCyan)
					dim.Println("No workspaces found.")
					fmt.Println()
					fmt.Println("Create a workspace with:")
					fmt.Print("  ")
					cyan.Println("wtp create <workspace_name>")
					fmt.Println()
					fmt.Println("All workspaces are stored under workspace_root (default: ~/.wtp/workspaces)")
				}
				return nil
			}

			if short {
				return lsShort(workspaces)
			}
			if long {
				return lsLong(workspaces)
			}
			return lsDefault(workspaces)
		},
	}

	cmd.Flags().BoolVarP(&long, "long", "l", false, "Show detailed information including repo status")
	cmd.Flags().BoolVarP(&short, "short", "s", false, "Output only workspace names (for shell completion)")

	return cmd
}

func lsShort(workspaces []core.WorkspaceInfo) error {
	for _, ws := range workspaces {
		fmt.Println(ws.Name)
	}
	return nil
}

func lsDefault(workspaces []core.WorkspaceInfo) error {
	cyan := color.New(color.FgCyan, color.Bold)
	dim := color.New(color.Faint)
	red := color.New(color.FgRed)

	for _, ws := range workspaces {
		name := cyan.Sprintf("%s", ws.Name)

		if !ws.Exists || !dirHasWtp(ws.Path) {
			fmt.Printf("%s  ", name)
			red.Println("[missing]")
			continue
		}

		repoInfo := "(error)"
		mgr, err := core.LoadWorktreeManager(ws.Path, nil)
		if err == nil {
			count := len(mgr.ListWorktrees())
			switch count {
			case 0:
				repoInfo = "(no repos)"
			case 1:
				repoInfo = "(1 repo)"
			default:
				repoInfo = fmt.Sprintf("(%d repos)", count)
			}
		}

		fmt.Printf("%s  ", name)
		dim.Println(repoInfo)
	}
	return nil
}

func lsLong(workspaces []core.WorkspaceInfo) error {
	git := core.NewGitClient()
	cyan := color.New(color.FgCyan, color.Bold)
	red := color.New(color.FgRed)
	dim := color.New(color.Faint)

	for i, ws := range workspaces {
		if i > 0 {
			fmt.Println()
		}

		if !ws.Exists || !dirHasWtp(ws.Path) {
			cyan.Print(ws.Name)
			fmt.Print("  ")
			red.Println("[missing]")
			continue
		}

		cyan.Println(ws.Name)

		mgr, err := core.LoadWorktreeManager(ws.Path, nil)
		if err != nil {
			fmt.Print("  ")
			red.Println("(error loading worktrees)")
			continue
		}

		worktrees := mgr.ListWorktrees()
		if len(worktrees) == 0 {
			fmt.Print("  ")
			dim.Println("(no repos)")
			continue
		}

		for _, wt := range worktrees {
			wtFullPath := filepath.Join(ws.Path, wt.WorktreePath)
			repoDisplay := wt.Repo.Display()

			if !pathExists(wtFullPath) {
				fmt.Printf("  %-30s %-20s ", repoDisplay, color.CyanString("%s", wt.Branch))
				red.Println("? missing")
				continue
			}

			statusStr := "?"
			if status, err := git.GetStatus(wtFullPath); err == nil {
				statusStr = status.FormatCompact()
			}

			fmt.Printf("  %-30s %-20s %s\n", repoDisplay, color.CyanString("%s", wt.Branch), statusStr)
		}
	}
	return nil
}

// dirHasWtp checks if a directory has a .wtp subdirectory.
func dirHasWtp(path string) bool {
	return pathIsDir(filepath.Join(path, core.WtpDir))
}

// pathExists checks if a path exists.
func pathExists(path string) bool {
	_, err := os.Stat(path)
	return err == nil
}

// pathIsDir checks if a path exists and is a directory.
func pathIsDir(path string) bool {
	info, err := os.Stat(path)
	return err == nil && info.IsDir()
}

func init() {
	rootCmd.AddCommand(NewLsCmd())
}
