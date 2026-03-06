package cmd

import (
	"fmt"

	"github.com/eddix/wtp/internal/core"
	"github.com/fatih/color"
	"github.com/spf13/cobra"
)

// NewCreateCmd returns the "wtp create" command.
func NewCreateCmd() *cobra.Command {
	var noHook bool

	cmd := &cobra.Command{
		Use:     "create <NAME>",
		Short:   "Create a new workspace",
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
			workspacePath, err := manager.CreateWorkspace(name, !noHook)
			if err != nil {
				return err
			}

			green := color.New(color.FgGreen, color.Bold)
			cyan := color.New(color.FgCyan)
			dim := color.New(color.Faint)

			green.Print("\u2713 ")
			fmt.Printf("Created workspace '")
			cyan.Print(name)
			fmt.Printf("' at ")
			dim.Println(workspacePath)
			fmt.Println()
			fmt.Println("To use this workspace, run:")
			fmt.Print("  ")
			cyan.Printf("cd %s\n", workspacePath)

			return nil
		},
	}

	cmd.Flags().BoolVar(&noHook, "no-hook", false, "Skip running the on_create hook script")

	return cmd
}

func init() {
	rootCmd.AddCommand(NewCreateCmd())
}
