package cmd

import (
	"fmt"
	"strings"

	"github.com/fatih/color"
	"github.com/spf13/cobra"
	"github.com/spf13/pflag"
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

// customHelpFunc prints a styled help message with grouped subcommands,
// matching the Rust version's output style.
func customHelpFunc(cmd *cobra.Command, args []string) {
	bold := color.New(color.Bold)
	cyan := color.New(color.FgCyan, color.Bold)
	blue := color.New(color.FgBlue)
	magenta := color.New(color.FgMagenta)

	// If this is the root command, print the full grouped help.
	if cmd == rootCmd {
		printRootHelp(cmd, bold, cyan, blue, magenta)
		return
	}

	// For subcommands, print styled help.
	printSubcommandHelp(cmd, bold, cyan, blue, magenta)
}

func printRootHelp(cmd *cobra.Command, bold, cyan, blue, magenta *color.Color) {
	// Title
	fmt.Println(cmd.Long)
	fmt.Println()

	// Usage
	bold.Print("Usage: ")
	cyan.Print("wtp ")
	magenta.Println("[OPTIONS] <COMMAND>")
	fmt.Println()

	// Grouped commands
	groups := cmd.Groups()
	for _, group := range groups {
		cmds := groupCommands(cmd, group.ID)
		if len(cmds) == 0 {
			continue
		}
		bold.Printf("%s:\n", group.Title)
		maxLen := maxCommandNameLen(cmds)
		for _, sub := range cmds {
			padding := strings.Repeat(" ", maxLen-len(sub.Name()))
			fmt.Print("  ")
			cyan.Printf("%s", sub.Name())
			fmt.Printf("%s  %s\n", padding, sub.Short)
		}
		fmt.Println()
	}

	// Ungrouped commands (if any)
	ungrouped := ungroupedCommands(cmd)
	if len(ungrouped) > 0 {
		bold.Println("Other Commands:")
		maxLen := maxCommandNameLen(ungrouped)
		for _, sub := range ungrouped {
			padding := strings.Repeat(" ", maxLen-len(sub.Name()))
			fmt.Print("  ")
			cyan.Printf("%s", sub.Name())
			fmt.Printf("%s  %s\n", padding, sub.Short)
		}
		fmt.Println()
	}

	// Options
	bold.Println("Options:")
	blue.Print("  -v, --verbose")
	fmt.Println("  Enable verbose output")
	blue.Print("  -h, --help")
	fmt.Println("     Print help")
	blue.Print("      --version")
	fmt.Println("  Print version")
	fmt.Println()
}

func printSubcommandHelp(cmd *cobra.Command, bold, cyan, blue, magenta *color.Color) {
	// About
	if cmd.Long != "" {
		fmt.Println(cmd.Long)
	} else if cmd.Short != "" {
		fmt.Println(cmd.Short)
	}
	fmt.Println()

	// Usage
	bold.Print("Usage: ")
	cmdPath := cmd.CommandPath()
	cyan.Printf("%s ", cmdPath)

	var usageParts []string
	if cmd.HasAvailableLocalFlags() {
		usageParts = append(usageParts, "[OPTIONS]")
	}
	if cmd.HasAvailableSubCommands() {
		usageParts = append(usageParts, "<COMMAND>")
	}
	// Add positional args from Use string (everything after the command name).
	parts := strings.Fields(cmd.Use)
	if len(parts) > 1 {
		usageParts = append(usageParts, parts[1:]...)
	}
	magenta.Println(strings.Join(usageParts, " "))
	fmt.Println()

	// Subcommands
	if cmd.HasAvailableSubCommands() {
		bold.Println("Commands:")
		subs := cmd.Commands()
		maxLen := maxCommandNameLen(subs)
		for _, sub := range subs {
			if sub.IsAvailableCommand() {
				padding := strings.Repeat(" ", maxLen-len(sub.Name()))
				fmt.Print("  ")
				cyan.Printf("%s", sub.Name())
				fmt.Printf("%s  %s\n", padding, sub.Short)
			}
		}
		fmt.Println()
	}

	// Local flags
	localFlags := cmd.LocalFlags()
	if localFlags.HasFlags() {
		bold.Println("Options:")
		localFlags.VisitAll(func(f *pflag.Flag) {
			printFlag(f, blue, magenta)
		})
		fmt.Println()
	}

	// Inherited flags
	inheritedFlags := cmd.InheritedFlags()
	if inheritedFlags.HasFlags() {
		bold.Println("Global Options:")
		inheritedFlags.VisitAll(func(f *pflag.Flag) {
			printFlag(f, blue, magenta)
		})
		fmt.Println()
	}
}

func printFlag(f *pflag.Flag, blue, magenta *color.Color) {
	var flagStr string
	if f.Shorthand != "" {
		flagStr = fmt.Sprintf("-%s, --%s", f.Shorthand, f.Name)
	} else {
		flagStr = fmt.Sprintf("    --%s", f.Name)
	}

	fmt.Print("  ")
	blue.Print(flagStr)

	if f.DefValue != "" && f.DefValue != "false" && f.DefValue != "true" {
		magenta.Printf(" <%s>", strings.ToUpper(f.Name))
	}

	fmt.Printf("  %s\n", f.Usage)
}

func groupCommands(cmd *cobra.Command, groupID string) []*cobra.Command {
	var result []*cobra.Command
	for _, sub := range cmd.Commands() {
		if sub.GroupID == groupID && sub.IsAvailableCommand() {
			result = append(result, sub)
		}
	}
	return result
}

func ungroupedCommands(cmd *cobra.Command) []*cobra.Command {
	var result []*cobra.Command
	for _, sub := range cmd.Commands() {
		if sub.GroupID == "" && sub.IsAvailableCommand() && sub.Name() != "help" {
			result = append(result, sub)
		}
	}
	return result
}

func maxCommandNameLen(cmds []*cobra.Command) int {
	maxLen := 0
	for _, c := range cmds {
		if len(c.Name()) > maxLen {
			maxLen = len(c.Name())
		}
	}
	return maxLen
}
