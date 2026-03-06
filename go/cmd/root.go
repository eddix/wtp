package cmd

import (
	"github.com/spf13/cobra"
)

// Command group constants for organizing help output.
const (
	GroupWorkspace  = "workspace"
	GroupRepository = "repository"
	GroupUtilities  = "utilities"
)

var verbose bool

var rootCmd = &cobra.Command{
	Use:     "wtp",
	Short:   "WorkTree for Polyrepo",
	Long:    "WorkTree for Polyrepo - Manage multiple git worktrees across repositories",
	Version: "0.1.0",
	// Disable default completion command.
	CompletionOptions: cobra.CompletionOptions{
		DisableDefaultCmd: true,
	},
	// Silence usage/errors so we handle them ourselves.
	SilenceUsage:  true,
	SilenceErrors: true,
}

func init() {
	rootCmd.PersistentFlags().BoolVarP(&verbose, "verbose", "v", false, "Enable verbose output")

	// Define command groups.
	rootCmd.AddGroup(
		&cobra.Group{ID: GroupWorkspace, Title: "Workspace Management"},
		&cobra.Group{ID: GroupRepository, Title: "Repository Operations"},
		&cobra.Group{ID: GroupUtilities, Title: "Utilities"},
	)

	// Override the default help function with our custom grouped help.
	rootCmd.SetHelpFunc(customHelpFunc)
}

// Execute runs the root command.
func Execute() error {
	return rootCmd.Execute()
}
