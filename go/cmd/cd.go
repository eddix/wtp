package cmd

import (
	"fmt"
	"os"

	"github.com/eddix/wtp/internal/core"
	"github.com/fatih/color"
	"github.com/spf13/cobra"
)

// NewCdCmd returns the "wtp cd" command.
func NewCdCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:     "cd [WORKSPACE]",
		Short:   "Change to a workspace directory",
		Long:    "Change to a workspace directory. Requires shell integration (eval \"$(wtp shell-init)\").",
		GroupID: GroupWorkspace,
		Args:    cobra.MaximumNArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			loadedConfig, warning, err := core.LoadConfig()
			if err != nil {
				return err
			}
			if warning != "" {
				color.Yellow("%s", warning)
			}

			manager := core.NewWorkspaceManager(loadedConfig)

			// Resolve workspace name.
			var workspaceName string
			if len(args) > 0 {
				workspaceName = args[0]
			} else {
				name, err := resolveWorkspaceInteractively(manager, "wtp cd")
				if err != nil {
					return err
				}
				workspaceName = name
			}

			// Get workspace path.
			workspacePath, ok := manager.GlobalConfig().GetWorkspacePath(workspaceName)
			if !ok {
				return fmt.Errorf(
					"workspace '%s' not found. Create it with: wtp create %s",
					workspaceName, workspaceName,
				)
			}

			if info, err := os.Stat(workspacePath); err != nil || !info.IsDir() {
				return fmt.Errorf(
					"workspace '%s' directory does not exist at %s",
					workspaceName, workspacePath,
				)
			}

			// Check if we're running inside the shell wrapper.
			directiveFile := os.Getenv("WTP_DIRECTIVE_FILE")

			if directiveFile != "" {
				// Write cd command to the directive file.
				cdCommand := fmt.Sprintf("cd '%s'", workspacePath)
				if err := os.WriteFile(directiveFile, []byte(cdCommand), 0644); err != nil {
					return fmt.Errorf("failed to write directive file: %w", err)
				}

				green := color.New(color.FgGreen, color.Bold)
				cyan := color.New(color.FgCyan)
				dim := color.New(color.Faint)

				green.Fprint(os.Stderr, "\u2713 ")
				fmt.Fprint(os.Stderr, "Changed to workspace '")
				cyan.Fprint(os.Stderr, workspaceName)
				fmt.Fprint(os.Stderr, "' at ")
				dim.Fprintln(os.Stderr, workspacePath)
			} else {
				// Not running in wrapper mode.
				red := color.New(color.FgRed, color.Bold)
				cyan := color.New(color.FgCyan)

				red.Fprintln(os.Stderr, "Error: wtp cd requires shell integration")
				fmt.Fprintln(os.Stderr)
				fmt.Fprintln(os.Stderr, "To enable 'wtp cd', add the following to your shell config:")
				fmt.Fprintln(os.Stderr)
				fmt.Fprint(os.Stderr, "  ")
				cyan.Fprintln(os.Stderr, "eval \"$(wtp shell-init)\"")
				fmt.Fprintln(os.Stderr)
				fmt.Fprintln(os.Stderr, "Or manually change to the workspace:")
				fmt.Fprint(os.Stderr, "  ")
				cyan.Fprintf(os.Stderr, "cd %s\n", workspacePath)

				return fmt.Errorf("shell integration not configured")
			}

			return nil
		},
	}

	return cmd
}

func init() {
	rootCmd.AddCommand(NewCdCmd())
}
