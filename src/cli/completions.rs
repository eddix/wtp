//! Shell completions command
//!
//! Generate shell completion scripts for zsh, bash, and fish.
//! Usage: eval "$(wtp completions zsh)"

use clap::{Args, ValueEnum};

#[derive(Debug, Clone, ValueEnum)]
pub enum ShellType {
    Zsh,
    Bash,
    Fish,
}

#[derive(Args, Debug)]
pub struct CompletionsArgs {
    /// Shell type
    #[arg(value_name = "SHELL")]
    shell: ShellType,
}

pub async fn execute(args: CompletionsArgs) -> anyhow::Result<()> {
    let script = match args.shell {
        ShellType::Zsh => generate_zsh(),
        ShellType::Bash => generate_bash(),
        ShellType::Fish => generate_fish(),
    };
    println!("{}", script);
    Ok(())
}

fn generate_zsh() -> String {
    r#"#compdef wtp

_wtp() {
    local -a commands
    local curcontext="$curcontext" state line

    _arguments -C \
        '-v[Enable verbose output]' \
        '--verbose[Enable verbose output]' \
        '-V[Print version]' \
        '--version[Print version]' \
        '-h[Print help]' \
        '--help[Print help]' \
        '1:command:->command' \
        '*::arg:->args'

    case $state in
        command)
            commands=(
                'cd:Change to a workspace directory'
                'create:Create a new workspace'
                'ls:List all workspaces'
                'remove:Remove a workspace'
                'status:Show status of a workspace'
                'config:Show current configuration'
                'eject:Eject a repository worktree from a workspace'
                'import:Import a repository worktree into a workspace'
                'switch:Switch current repo to a workspace'
                'host:Manage host aliases'
                'shell-init:Generate shell integration script'
                'completions:Generate shell completion scripts'
            )
            _describe 'wtp commands' commands
            ;;
        args)
            case $line[1] in
                cd)
                    _arguments \
                        '1:workspace:_wtp_workspaces'
                    ;;
                create)
                    _arguments \
                        '--no-hook[Skip post-create hook]' \
                        '1:name:'
                    ;;
                ls)
                    _arguments \
                        '-l[Show detailed information]' \
                        '--long[Show detailed information]' \
                        '-s[Output only workspace names]' \
                        '--short[Output only workspace names]'
                    ;;
                remove)
                    _arguments \
                        '-f[Force removal]' \
                        '--force[Force removal]' \
                        '1:workspace:_wtp_workspaces'
                    ;;
                status)
                    _arguments \
                        '-l[Show detailed information]' \
                        '--long[Show detailed information]'
                    ;;
                eject)
                    _arguments \
                        '-f[Force eject]' \
                        '--force[Force eject]' \
                        '1:repository:'
                    ;;
                import)
                    _arguments \
                        '-H[Host alias]:alias:_wtp_hosts' \
                        '--host[Host alias]:alias:_wtp_hosts' \
                        '-b[Branch name]:branch:' \
                        '--branch[Branch name]:branch:' \
                        '-B[Base reference]:base:' \
                        '--base[Base reference]:base:' \
                        '--repo[Full repository path]:path:_files -/' \
                        '1:path:_files -/'
                    ;;
                switch)
                    _arguments \
                        '1:workspace:_wtp_workspaces'
                    ;;
                host)
                    local -a host_commands
                    host_commands=(
                        'add:Add a new host alias'
                        'ls:List all configured hosts'
                        'rm:Remove a host alias'
                        'set-default:Set the default host'
                    )
                    _arguments -C \
                        '1:host command:->hostcmd' \
                        '*::arg:->hostargs'
                    case $state in
                        hostcmd)
                            _describe 'host commands' host_commands
                            ;;
                        hostargs)
                            case $line[1] in
                                add)
                                    _arguments \
                                        '1:alias:' \
                                        '2:path:_files -/'
                                    ;;
                                rm|set-default)
                                    _arguments \
                                        '1:alias:_wtp_hosts'
                                    ;;
                            esac
                            ;;
                    esac
                    ;;
                completions)
                    _arguments \
                        '1:shell:(zsh bash fish)'
                    ;;
            esac
            ;;
    esac
}

_wtp_workspaces() {
    local -a workspaces
    workspaces=(${(f)"$(command wtp ls --short 2>/dev/null)"})
    _describe 'workspaces' workspaces
}

_wtp_hosts() {
    local -a hosts
    hosts=(${(f)"$(command wtp host ls 2>/dev/null | grep -oP '^\s+\K\S+(?=\s+->)')"})
    _describe 'hosts' hosts
}

compdef _wtp wtp"#
        .to_string()
}

fn generate_bash() -> String {
    r#"# bash completion for wtp

_wtp_completions() {
    local cur prev words cword
    _init_completion || return

    local commands="cd create ls remove status config eject import switch host shell-init completions"
    local host_commands="add ls rm set-default"

    if [[ $cword -eq 1 ]]; then
        COMPREPLY=($(compgen -W "$commands" -- "$cur"))
        return
    fi

    case "${words[1]}" in
        cd|remove|switch)
            local workspaces
            workspaces="$(command wtp ls --short 2>/dev/null)"
            COMPREPLY=($(compgen -W "$workspaces" -- "$cur"))
            ;;
        status)
            if [[ "$cur" == -* ]]; then
                COMPREPLY=($(compgen -W "-l --long" -- "$cur"))
            fi
            ;;
        eject)
            if [[ "$cur" == -* ]]; then
                COMPREPLY=($(compgen -W "-f --force" -- "$cur"))
            fi
            ;;
        import)
            if [[ "$cur" == -* ]]; then
                COMPREPLY=($(compgen -W "-H --host -b --branch -B --base --repo" -- "$cur"))
            elif [[ "$prev" == "--repo" ]]; then
                _filedir -d
            else
                _filedir -d
            fi
            ;;
        create)
            if [[ "$cur" == -* ]]; then
                COMPREPLY=($(compgen -W "--no-hook" -- "$cur"))
            fi
            ;;
        ls)
            COMPREPLY=($(compgen -W "-l --long -s --short" -- "$cur"))
            ;;
        remove)
            if [[ "$cur" == -* ]]; then
                COMPREPLY=($(compgen -W "-f --force" -- "$cur"))
            else
                local workspaces
                workspaces="$(command wtp ls --short 2>/dev/null)"
                COMPREPLY=($(compgen -W "$workspaces" -- "$cur"))
            fi
            ;;
        host)
            if [[ $cword -eq 2 ]]; then
                COMPREPLY=($(compgen -W "$host_commands" -- "$cur"))
            else
                case "${words[2]}" in
                    add)
                        if [[ $cword -eq 4 ]]; then
                            _filedir -d
                        fi
                        ;;
                    rm|set-default)
                        local hosts
                        hosts="$(command wtp host ls 2>/dev/null | grep -oP '^\s+\K\S+(?=\s+->)')"
                        COMPREPLY=($(compgen -W "$hosts" -- "$cur"))
                        ;;
                esac
            fi
            ;;
        completions)
            COMPREPLY=($(compgen -W "zsh bash fish" -- "$cur"))
            ;;
    esac
}

complete -F _wtp_completions wtp"#
        .to_string()
}

fn generate_fish() -> String {
    r#"# fish completion for wtp

# Disable file completion by default
complete -c wtp -f

# Global options
complete -c wtp -s v -l verbose -d 'Enable verbose output'
complete -c wtp -s V -l version -d 'Print version'
complete -c wtp -s h -l help -d 'Print help'

# Main commands
complete -c wtp -n '__fish_use_subcommand' -a cd -d 'Change to a workspace directory'
complete -c wtp -n '__fish_use_subcommand' -a create -d 'Create a new workspace'
complete -c wtp -n '__fish_use_subcommand' -a ls -d 'List all workspaces'
complete -c wtp -n '__fish_use_subcommand' -a remove -d 'Remove a workspace'
complete -c wtp -n '__fish_use_subcommand' -a status -d 'Show status of a workspace'
complete -c wtp -n '__fish_use_subcommand' -a config -d 'Show current configuration'
complete -c wtp -n '__fish_use_subcommand' -a eject -d 'Eject a repository worktree from a workspace'
complete -c wtp -n '__fish_use_subcommand' -a import -d 'Import a repository worktree into a workspace'
complete -c wtp -n '__fish_use_subcommand' -a switch -d 'Switch current repo to a workspace'
complete -c wtp -n '__fish_use_subcommand' -a host -d 'Manage host aliases'
complete -c wtp -n '__fish_use_subcommand' -a shell-init -d 'Generate shell integration script'
complete -c wtp -n '__fish_use_subcommand' -a completions -d 'Generate shell completion scripts'

# cd - complete with workspace names
complete -c wtp -n '__fish_seen_subcommand_from cd' -a '(command wtp ls --short 2>/dev/null)'

# create
complete -c wtp -n '__fish_seen_subcommand_from create' -l no-hook -d 'Skip post-create hook'

# ls
complete -c wtp -n '__fish_seen_subcommand_from ls' -s l -l long -d 'Show detailed information'
complete -c wtp -n '__fish_seen_subcommand_from ls' -s s -l short -d 'Output only workspace names'

# remove
complete -c wtp -n '__fish_seen_subcommand_from remove' -s f -l force -d 'Force removal'
complete -c wtp -n '__fish_seen_subcommand_from remove' -a '(command wtp ls --short 2>/dev/null)'

# status
complete -c wtp -n '__fish_seen_subcommand_from status' -s l -l long -d 'Show detailed information'

# eject
complete -c wtp -n '__fish_seen_subcommand_from eject' -s f -l force -d 'Force eject'

# import
complete -c wtp -n '__fish_seen_subcommand_from import' -s H -l host -d 'Host alias' -r
complete -c wtp -n '__fish_seen_subcommand_from import' -s b -l branch -d 'Branch name' -r
complete -c wtp -n '__fish_seen_subcommand_from import' -s B -l base -d 'Base reference' -r
complete -c wtp -n '__fish_seen_subcommand_from import' -l repo -d 'Full repository path' -r -F
complete -c wtp -n '__fish_seen_subcommand_from import' -F

# switch
complete -c wtp -n '__fish_seen_subcommand_from switch' -a '(command wtp ls --short 2>/dev/null)'

# host subcommands
complete -c wtp -n '__fish_seen_subcommand_from host; and not __fish_seen_subcommand_from add ls rm set-default' -a add -d 'Add a new host alias'
complete -c wtp -n '__fish_seen_subcommand_from host; and not __fish_seen_subcommand_from add ls rm set-default' -a ls -d 'List all configured hosts'
complete -c wtp -n '__fish_seen_subcommand_from host; and not __fish_seen_subcommand_from add ls rm set-default' -a rm -d 'Remove a host alias'
complete -c wtp -n '__fish_seen_subcommand_from host; and not __fish_seen_subcommand_from add ls rm set-default' -a set-default -d 'Set the default host'

# completions
complete -c wtp -n '__fish_seen_subcommand_from completions' -a 'zsh bash fish'"#
        .to_string()
}
