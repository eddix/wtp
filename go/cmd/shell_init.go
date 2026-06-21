package cmd

import (
	"fmt"

	"github.com/spf13/cobra"
)

// NewShellInitCmd returns the "wtp shell-init" command.
func NewShellInitCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:     "shell-init",
		Short:   "Output shell integration script",
		Long:    "Output shell wrapper script for eval. Usage: eval \"$(wtp shell-init)\"",
		GroupID: GroupUtilities,
		Args:    cobra.NoArgs,
		RunE: func(cmd *cobra.Command, args []string) error {
			fmt.Println(generateShellWrapper())
			return nil
		},
	}

	return cmd
}

func generateShellWrapper() string {
	return `# wtp shell wrapper
# Add this to your shell config: eval "$(wtp shell-init)"

wtp() {
    local tmpfile=""

    # Set up directive file for cd command
    if [[ "$1" == "cd" ]]; then
        tmpfile=$(mktemp "${TMPDIR:-/tmp}/wtp.XXXXXX")
        export WTP_DIRECTIVE_FILE="$tmpfile"
    fi

    # Run the actual wtp binary
    command wtp "$@"
    local exit_code=$?

    # Source directive file if it exists and has content
    if [[ -n "$tmpfile" && -s "$tmpfile" ]]; then
        # shellcheck source=/dev/null
        source "$tmpfile"
        rm -f "$tmpfile"
        unset WTP_DIRECTIVE_FILE
    elif [[ -n "$tmpfile" ]]; then
        rm -f "$tmpfile"
        unset WTP_DIRECTIVE_FILE
    fi

    return $exit_code
}

# Enable tab completion for wtp cd
if command -v compdef &> /dev/null; then
    # zsh
    _wtp_complete() {
        local -a workspaces
        workspaces=(${(f)"$(command wtp ls --short 2>/dev/null)"})
        _describe 'workspaces' workspaces
    }
    compdef _wtp_complete wtp
fi`
}

func init() {
	rootCmd.AddCommand(NewShellInitCmd())
}
