//! Shell integration - generates shell wrapper scripts
//!
//! This enables `wtp cd` to work by having the parent shell source
//! commands from a temporary file after wtp exits.
//! Currently supports bash and zsh.

use clap::Args;

#[derive(Args, Debug)]
pub struct ShellInitArgs {
    /// Shell type (ignored, kept for backward compatibility)
    #[arg(value_name = "SHELL", hide = true)]
    shell: Option<String>,
}

pub async fn execute(_args: ShellInitArgs) -> anyhow::Result<()> {
    let script = generate_shell_wrapper();
    println!("{}", script);
    Ok(())
}

/// Generate the shell wrapper script (bash/zsh)
///
/// Usage: eval "$(wtp shell-init)"
fn generate_shell_wrapper() -> String {
    r#"# wtp shell wrapper (bash/zsh)
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
"#.to_string()
}
