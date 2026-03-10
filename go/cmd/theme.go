package cmd

import "github.com/fatih/color"

// Style definitions for consistent output across all commands.
// These mirror the Rust version's theme (src/cli/theme.rs).
var (
	// StyleBold is used for section headers (e.g., "Usage:", "Options:").
	StyleBold = color.New(color.Bold)

	// StyleCyan is used for command names, workspace names, branch names.
	StyleCyan = color.New(color.FgCyan, color.Bold)

	// StyleBlue is used for flag names (e.g., "--force", "-v").
	StyleBlue = color.New(color.FgBlue)

	// StyleMagenta is used for placeholders and value names (e.g., "<NAME>").
	StyleMagenta = color.New(color.FgMagenta)

	// StyleGreen is used for success indicators (e.g., checkmarks).
	StyleGreen = color.New(color.FgGreen, color.Bold)

	// StyleYellow is used for warnings and dirty status.
	StyleYellow = color.New(color.FgYellow)

	// StyleRed is used for errors and missing items.
	StyleRed = color.New(color.FgRed, color.Bold)

	// StyleDimmed is used for secondary information (paths, timestamps).
	StyleDimmed = color.New(color.Faint)
)
