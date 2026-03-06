package cmd

import (
	"fmt"
	"os"
	"sort"
	"strings"

	"github.com/eddix/wtp/internal/core"
	"github.com/fatih/color"
	"github.com/spf13/cobra"
)

// NewHostCmd returns the "wtp host" command with subcommands.
func NewHostCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:     "host",
		Short:   "Manage host aliases",
		GroupID: GroupUtilities,
	}

	cmd.AddCommand(newHostAddCmd())
	cmd.AddCommand(newHostLsCmd())
	cmd.AddCommand(newHostRmCmd())
	cmd.AddCommand(newHostSetDefaultCmd())

	return cmd
}

func newHostAddCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "add <ALIAS> <PATH>",
		Short: "Add a new host alias",
		Args:  cobra.ExactArgs(2),
		RunE: func(cmd *cobra.Command, args []string) error {
			alias := args[0]
			path := args[1]

			// Validate alias.
			if strings.ContainsAny(alias, " /\\") {
				return fmt.Errorf(
					"host alias '%s' contains invalid characters. Use simple names like 'gh', 'gl', 'bb'",
					alias,
				)
			}

			// Expand tilde.
			expanded := core.ExpandTilde(path)

			loadedConfig, warning, err := core.LoadConfig()
			if err != nil {
				return err
			}
			if warning != "" {
				color.Yellow("%s", warning)
			}

			// Check if path exists.
			if info, err := os.Stat(expanded); err != nil || !info.IsDir() {
				yellow := color.New(color.FgYellow)
				yellow.Fprintf(os.Stderr, "Warning: Path '%s' does not exist yet.\n", expanded)
				fmt.Fprintln(os.Stderr, "The host will be added, but repositories under it won't be accessible until the directory is created.")
			}

			// Check duplicate.
			if _, ok := loadedConfig.Config.GetHostRoot(alias); ok {
				return fmt.Errorf(
					"host alias '%s' already exists. Use 'wtp host rm %s' first if you want to replace it",
					alias, alias,
				)
			}

			// Add host.
			if loadedConfig.Config.Hosts == nil {
				loadedConfig.Config.Hosts = make(map[string]core.HostConfig)
			}
			loadedConfig.Config.Hosts[alias] = core.HostConfig{Root: expanded}

			if err := loadedConfig.Save(); err != nil {
				return err
			}

			green := color.New(color.FgGreen, color.Bold)
			cyan := color.New(color.FgCyan)
			dim := color.New(color.Faint)

			green.Print("\u2713 ")
			fmt.Print("Added host alias '")
			cyan.Print(alias)
			fmt.Print("' -> ")
			dim.Println(expanded)

			return nil
		},
	}
}

func newHostLsCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "ls",
		Short: "List all configured hosts",
		Args:  cobra.NoArgs,
		RunE: func(cmd *cobra.Command, args []string) error {
			loadedConfig, warning, err := core.LoadConfig()
			if err != nil {
				return err
			}
			if warning != "" {
				color.Yellow("%s", warning)
			}

			hosts := loadedConfig.Config.Hosts
			defaultHost := loadedConfig.Config.DefaultHostAlias()

			if len(hosts) == 0 {
				dim := color.New(color.Faint)
				cyan := color.New(color.FgCyan)

				dim.Println("No host aliases configured.")
				fmt.Println()
				fmt.Println("Add a host with:")
				fmt.Print("  ")
				cyan.Println("wtp host add <alias> <path>")
				fmt.Println()
				fmt.Println("Example:")
				fmt.Print("  ")
				cyan.Println("wtp host add gh ~/codes/github.com")
				return nil
			}

			bold := color.New(color.Bold)
			cyan := color.New(color.FgCyan, color.Bold)
			dim := color.New(color.Faint)
			green := color.New(color.FgGreen)

			bold.Println("Configured hosts:")
			fmt.Println()

			// Sort aliases for stable output.
			aliases := make([]string, 0, len(hosts))
			for alias := range hosts {
				aliases = append(aliases, alias)
			}
			sort.Strings(aliases)

			for _, alias := range aliases {
				config := hosts[alias]
				fmt.Print("  ")
				cyan.Print(alias)
				fmt.Print(" -> ")
				dim.Print(config.Root)
				if defaultHost != "" && alias == defaultHost {
					green.Print(" (default)")
				}
				fmt.Println()
			}

			if defaultHost == "" {
				fmt.Println()
				dim.Println("No default host set. Use 'wtp host set-default <alias>' to set one.")
			}

			return nil
		},
	}
}

func newHostRmCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "rm <ALIAS>",
		Short: "Remove a host alias",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			alias := args[0]

			loadedConfig, warning, err := core.LoadConfig()
			if err != nil {
				return err
			}
			if warning != "" {
				color.Yellow("%s", warning)
			}

			// Check if host exists.
			if _, ok := loadedConfig.Config.GetHostRoot(alias); !ok {
				return fmt.Errorf("host alias '%s' not found", alias)
			}

			// Check if this is the default host.
			if loadedConfig.Config.DefaultHostAlias() == alias {
				yellow := color.New(color.FgYellow)
				yellow.Fprintf(os.Stderr, "'%s' is currently the default host. It will be unset.\n", alias)
				loadedConfig.Config.DefaultHost = ""
			}

			delete(loadedConfig.Config.Hosts, alias)

			if err := loadedConfig.Save(); err != nil {
				return err
			}

			green := color.New(color.FgGreen, color.Bold)
			cyan := color.New(color.FgCyan)

			green.Print("\u2713 ")
			fmt.Print("Removed host alias '")
			cyan.Print(alias)
			fmt.Println("'")

			return nil
		},
	}
}

func newHostSetDefaultCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "set-default <ALIAS>",
		Short: "Set the default host",
		Long:  "Set the default host alias. Use 'none', 'null', or '-' to unset.",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			alias := args[0]

			loadedConfig, warning, err := core.LoadConfig()
			if err != nil {
				return err
			}
			if warning != "" {
				color.Yellow("%s", warning)
			}

			green := color.New(color.FgGreen, color.Bold)

			// Special case: unset.
			if alias == "none" || alias == "null" || alias == "-" {
				loadedConfig.Config.DefaultHost = ""
				if err := loadedConfig.Save(); err != nil {
					return err
				}
				green.Print("\u2713 ")
				fmt.Println("Unset default host")
				return nil
			}

			// Check if host exists.
			if _, ok := loadedConfig.Config.GetHostRoot(alias); !ok {
				return fmt.Errorf(
					"host alias '%s' not found. Add it first with 'wtp host add %s <path>'",
					alias, alias,
				)
			}

			loadedConfig.Config.DefaultHost = alias

			if err := loadedConfig.Save(); err != nil {
				return err
			}

			cyan := color.New(color.FgCyan)
			green.Print("\u2713 ")
			fmt.Print("Set '")
			cyan.Print(alias)
			fmt.Println("' as default host")

			return nil
		},
	}
}

func init() {
	rootCmd.AddCommand(NewHostCmd())
}
