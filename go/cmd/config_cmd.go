package cmd

import (
	"fmt"
	"sort"

	"github.com/eddix/wtp/internal/core"
	"github.com/fatih/color"
	"github.com/spf13/cobra"
)

// NewConfigCmd returns the "wtp config" command.
func NewConfigCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:     "config",
		Short:   "Show current configuration",
		GroupID: GroupUtilities,
		Args:    cobra.NoArgs,
		RunE: func(cmd *cobra.Command, args []string) error {
			loadedConfig, warning, err := core.LoadConfig()
			if err != nil {
				return err
			}
			if warning != "" {
				color.Yellow("%s", warning)
			}

			config := &loadedConfig.Config

			cyan := color.New(color.FgCyan)
			green := color.New(color.FgGreen, color.Bold)
			dim := color.New(color.Faint)

			green.Println("Current Configuration")
			fmt.Println()

			// Config file source.
			if loadedConfig.SourcePath != "" {
				fmt.Printf("%s: %s\n", cyan.Sprint("Config file"), loadedConfig.SourcePath)
			} else {
				fmt.Printf("%s: %s\n", cyan.Sprint("Config file"), dim.Sprint("(default, not saved)"))
			}
			fmt.Println()

			// Workspace root.
			fmt.Printf("%s: %s\n", cyan.Sprint("Workspace root"), config.WorkspaceRoot)

			// Scan and list workspaces.
			workspaces := config.ScanWorkspaces()
			fmt.Printf("%s: %d found\n", cyan.Sprint("Workspaces"), len(workspaces))
			if len(workspaces) == 0 {
				fmt.Printf("  %s\n", dim.Sprint("(none)"))
			} else {
				// Sort workspace names for stable output.
				names := make([]string, 0, len(workspaces))
				for name := range workspaces {
					names = append(names, name)
				}
				sort.Strings(names)
				for _, name := range names {
					fmt.Printf("  %s: %s\n", color.GreenString("%s", name), workspaces[name])
				}
			}
			fmt.Println()

			// Hosts.
			fmt.Printf("%s: %d\n", cyan.Sprint("Hosts"), len(config.Hosts))
			if len(config.Hosts) == 0 {
				fmt.Printf("  %s\n", dim.Sprint("(none)"))
			} else {
				// Sort host aliases for stable output.
				aliases := make([]string, 0, len(config.Hosts))
				for alias := range config.Hosts {
					aliases = append(aliases, alias)
				}
				sort.Strings(aliases)
				for _, alias := range aliases {
					fmt.Printf("  %s: %s\n", color.GreenString("%s", alias), config.Hosts[alias].Root)
				}
			}

			// Default host.
			if config.DefaultHost != "" {
				fmt.Printf("%s: %s\n", cyan.Sprint("Default host"), color.GreenString("%s", config.DefaultHost))
			}
			fmt.Println()

			// Hooks.
			cyan.Println("Hooks")
			if config.Hooks.OnCreate != "" {
				fmt.Printf("  %s: %s\n", color.GreenString("on_create"), config.Hooks.OnCreate)
			} else {
				fmt.Printf("  %s: %s\n", color.GreenString("on_create"), dim.Sprint("(not set)"))
			}

			return nil
		},
	}

	return cmd
}

func init() {
	rootCmd.AddCommand(NewConfigCmd())
}
