//! Integration tests for wtp

use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

fn wtp_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_wtp"))
}

/// Setup a temporary home directory for testing to avoid polluting user's ~/.wtp
fn setup_test_env() -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    temp_dir
}

fn run_wtp_with_home(args: &[&str], home: &std::path::Path) -> (bool, String, String) {
    run_wtp_in_dir_with_home(args, None, home)
}

fn run_wtp_in_dir_with_home(
    args: &[&str],
    cwd: Option<&std::path::Path>,
    home: &std::path::Path,
) -> (bool, String, String) {
    let mut cmd = Command::new(wtp_bin());
    cmd.args(args);

    // Set HOME to temp directory to isolate test from user's config
    cmd.env("HOME", home);
    // Also set these to be thorough
    cmd.env_remove("XDG_CONFIG_HOME");

    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    let output = cmd.output().expect("Failed to execute wtp");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    (output.status.success(), stdout, stderr)
}

#[test]
fn test_wtp_help() {
    let temp_home = setup_test_env();

    let (success, stdout, _) = run_wtp_with_home(&["--help"], temp_home.path());
    assert!(success);
    assert!(stdout.contains("WorkTree for Polyrepo"));
    assert!(stdout.contains("cd"));
    assert!(stdout.contains("ls"));
    assert!(stdout.contains("create"));
    assert!(stdout.contains("import"));
    assert!(stdout.contains("switch"));
    assert!(stdout.contains("eject"));
    assert!(stdout.contains("shell-init"));
    assert!(stdout.contains("completions"));
    assert!(stdout.contains("Workspace Management"));
    assert!(stdout.contains("Repository Operations"));
    assert!(stdout.contains("Utilities"));
    assert!(!stdout.contains("  init  ")); // init command was removed (but shell-init exists)
}

#[test]
fn test_wtp_version() {
    let temp_home = setup_test_env();

    let (success, stdout, _) = run_wtp_with_home(&["--version"], temp_home.path());
    assert!(success);
    assert!(stdout.contains("0.1.0"));
}

#[test]
fn test_import_requires_workspace() {
    let temp_home = setup_test_env();
    let temp_dir = TempDir::new().unwrap();

    // Without --workspace and not in a workspace, should fail with "Not in a workspace"
    let (success, stdout, stderr) = run_wtp_in_dir_with_home(
        &["import", "some/path"],
        Some(temp_dir.path()),
        temp_home.path(),
    );
    assert!(!success);
    let combined = format!("{} {}", stdout, stderr);
    assert!(
        combined.contains("Not in a workspace") || combined.contains("workspace"),
        "Expected workspace-related error, got: {}",
        combined
    );
}

#[test]
fn test_status_not_in_workspace() {
    let temp_home = setup_test_env();
    let temp_dir = TempDir::new().unwrap();

    // Without --workspace and not in a workspace, should fail with "Not in a workspace"
    let (success, stdout, stderr) =
        run_wtp_in_dir_with_home(&["status"], Some(temp_dir.path()), temp_home.path());
    assert!(!success);
    let combined = format!("{} {}", stdout, stderr);
    assert!(
        combined.contains("Not in a workspace") || combined.contains("workspace"),
        "Expected workspace-related error, got: {}",
        combined
    );
}

#[test]
fn test_cd_requires_shell_integration() {
    let temp_home = setup_test_env();

    // Use a workspace name that definitely doesn't exist
    let (success, stdout, stderr) =
        run_wtp_with_home(&["cd", "nonexistent-workspace-xyz"], temp_home.path());
    assert!(!success);
    let combined = format!("{} {}", stdout, stderr);
    // Either "not found" or "shell integration" error is acceptable
    assert!(
        combined.contains("not found")
            || combined.contains("shell integration")
            || combined.contains("Shell integration"),
        "Expected error message, got: {}",
        combined
    );
}

#[test]
fn test_shell_init_outputs_wrapper() {
    let temp_home = setup_test_env();

    let (success, stdout, _) = run_wtp_with_home(&["shell-init"], temp_home.path());
    assert!(success);
    assert!(stdout.contains("wtp() {"));
    assert!(stdout.contains("WTP_DIRECTIVE_FILE"));
}

#[test]
fn test_ls_short_format() {
    let temp_home = setup_test_env();

    // First create a workspace in the isolated temp home
    let _ = run_wtp_with_home(&["create", "test-short"], temp_home.path());

    let (success, stdout, _) = run_wtp_with_home(&["ls", "--short"], temp_home.path());
    assert!(success);
    // Should contain our test workspace
    assert!(
        stdout.contains("test-short"),
        "Expected 'test-short' in output, got: {}",
        stdout
    );

    // Cleanup
    let _ = run_wtp_with_home(&["rm", "test-short", "--force"], temp_home.path());
}

#[test]
fn test_create_workspace_with_hook() {
    let temp_home = setup_test_env();
    let home_path = temp_home.path();

    // Create a hook script
    let hooks_dir = home_path.join(".wtp").join("hooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();

    let hook_script = hooks_dir.join("on-create.sh");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(
            &hook_script,
            r#"#!/bin/bash
echo "HOOK_RAN: $WTP_WORKSPACE_NAME"
touch "$WTP_WORKSPACE_PATH/hook-marker.txt"
"#,
        )
        .unwrap();
        let mut perms = std::fs::metadata(&hook_script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&hook_script, perms).unwrap();
    }
    #[cfg(not(unix))]
    {
        std::fs::write(
            &hook_script,
            r#"#!/bin/bash
echo "HOOK_RAN: $WTP_WORKSPACE_NAME"
touch "$WTP_WORKSPACE_PATH/hook-marker.txt"
"#,
        )
        .unwrap();
    }

    // Create config with hook
    let config_content = format!(
        r#"workspace_root = "{}/.wtp/workspaces"

[hooks]
on_create = "{}"
"#,
        home_path.display(),
        hook_script.display()
    );
    let wtp_dir = home_path.join(".wtp");
    std::fs::create_dir_all(&wtp_dir).unwrap();
    std::fs::write(wtp_dir.join("config.toml"), config_content).unwrap();

    // Create workspace - hook should run
    let (success, stdout, stderr) = run_wtp_with_home(&["create", "test-hook-ws"], home_path);
    assert!(success, "Failed to create workspace: {}", stderr);

    // Check hook output
    assert!(
        stdout.contains("HOOK_RAN: test-hook-ws"),
        "Expected hook output in stdout, got: {}",
        stdout
    );

    // Verify marker file was created by hook
    let workspace_path = home_path
        .join(".wtp")
        .join("workspaces")
        .join("test-hook-ws");
    let marker_file = workspace_path.join("hook-marker.txt");
    assert!(marker_file.exists(), "Hook marker file should exist");

    // Cleanup
    let _ = run_wtp_with_home(&["rm", "test-hook-ws", "--force"], home_path);
}

#[test]
fn test_create_workspace_skip_hook() {
    let temp_home = setup_test_env();
    let home_path = temp_home.path();

    // Create a hook script that would fail if run
    let hooks_dir = home_path.join(".wtp").join("hooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();

    let hook_script = hooks_dir.join("on-create.sh");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(
            &hook_script,
            r#"#!/bin/bash
echo "HOOK_SHOULD_NOT_RUN"
exit 1
"#,
        )
        .unwrap();
        let mut perms = std::fs::metadata(&hook_script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&hook_script, perms).unwrap();
    }
    #[cfg(not(unix))]
    {
        std::fs::write(
            &hook_script,
            r#"#!/bin/bash
echo "HOOK_SHOULD_NOT_RUN"
exit 1
"#,
        )
        .unwrap();
    }

    // Create config with hook
    let config_content = format!(
        r#"workspace_root = "{}/.wtp/workspaces"

[hooks]
on_create = "{}"
"#,
        home_path.display(),
        hook_script.display()
    );
    let wtp_dir = home_path.join(".wtp");
    std::fs::create_dir_all(&wtp_dir).unwrap();
    std::fs::write(wtp_dir.join("config.toml"), config_content).unwrap();

    // Create workspace with --no-hook - hook should NOT run
    let (success, stdout, stderr) =
        run_wtp_with_home(&["create", "test-no-hook-ws", "--no-hook"], home_path);
    assert!(success, "Failed to create workspace: {}", stderr);

    // Check hook output was NOT shown
    assert!(
        !stdout.contains("HOOK_SHOULD_NOT_RUN"),
        "Hook should not have run, but output contains hook text: {}",
        stdout
    );

    // Cleanup
    let _ = run_wtp_with_home(&["rm", "test-no-hook-ws", "--force"], home_path);
}

#[test]
fn test_eject_not_in_workspace() {
    let temp_home = setup_test_env();
    let temp_dir = TempDir::new().unwrap();

    let (success, stdout, stderr) = run_wtp_in_dir_with_home(
        &["eject", "some-repo"],
        Some(temp_dir.path()),
        temp_home.path(),
    );
    assert!(!success);
    let combined = format!("{} {}", stdout, stderr);
    assert!(
        combined.contains("Not in a workspace") || combined.contains("workspace"),
        "Expected workspace-related error, got: {}",
        combined
    );
}

#[test]
fn test_eject_help() {
    let temp_home = setup_test_env();

    let (success, stdout, _) = run_wtp_with_home(&["help", "eject"], temp_home.path());
    assert!(success);
    assert!(stdout.contains("Eject"));
    assert!(stdout.contains("Usage:"));
    assert!(stdout.contains("wtp eject"));
}

#[test]
fn test_completions_zsh() {
    let temp_home = setup_test_env();

    let (success, stdout, _) = run_wtp_with_home(&["completions", "zsh"], temp_home.path());
    assert!(success);
    assert!(stdout.contains("#compdef wtp"));
    assert!(stdout.contains("_wtp_workspaces"));
}

#[test]
fn test_completions_bash() {
    let temp_home = setup_test_env();

    let (success, stdout, _) = run_wtp_with_home(&["completions", "bash"], temp_home.path());
    assert!(success);
    assert!(stdout.contains("_wtp_completions"));
    assert!(stdout.contains("complete -F"));
}

#[test]
fn test_completions_fish() {
    let temp_home = setup_test_env();

    let (success, stdout, _) = run_wtp_with_home(&["completions", "fish"], temp_home.path());
    assert!(success);
    assert!(stdout.contains("complete -c wtp"));
}

#[test]
fn test_completions_invalid_shell() {
    let temp_home = setup_test_env();

    let (success, _, stderr) = run_wtp_with_home(&["completions", "powershell"], temp_home.path());
    assert!(!success);
    // clap's ValueEnum validation provides the error message
    assert!(
        stderr.contains("invalid value") || stderr.contains("possible values"),
        "Expected clap validation error, got: {}",
        stderr
    );
}

#[test]
fn test_create_workspace_sanitizes_slash() {
    let temp_home = setup_test_env();
    let home_path = temp_home.path();

    let (success, stdout, stderr) = run_wtp_with_home(
        &["create", "hotfix/update_task_issue_1234", "--no-hook"],
        home_path,
    );
    assert!(
        success,
        "create failed; stdout={} stderr={}",
        stdout, stderr
    );
    assert!(
        stdout.contains("hotfix_update_task_issue_1234"),
        "expected sanitized name in output, got: {}",
        stdout
    );
    assert!(
        stdout.contains("Sanitized") || stdout.contains("sanitized"),
        "expected 'Sanitized' notice in output, got: {}",
        stdout
    );

    // `wtp ls` should now find the workspace under its sanitized name.
    let (ok, ls_out, ls_err) = run_wtp_with_home(&["ls", "--short"], home_path);
    assert!(ok, "ls failed; stdout={} stderr={}", ls_out, ls_err);
    assert!(
        ls_out.contains("hotfix_update_task_issue_1234"),
        "ls did not find sanitized workspace, got: {}",
        ls_out
    );

    // The nested directory must NOT have been created.
    let nested = home_path
        .join(".wtp")
        .join("workspaces")
        .join("hotfix")
        .join("update_task_issue_1234");
    assert!(
        !nested.exists(),
        "nested directory was created: {}",
        nested.display()
    );

    // Cleanup
    let _ = run_wtp_with_home(
        &["rm", "hotfix_update_task_issue_1234", "--force"],
        home_path,
    );
}

#[test]
fn test_create_workspace_collapses_double_slash() {
    let temp_home = setup_test_env();
    let home_path = temp_home.path();

    let (success, stdout, stderr) =
        run_wtp_with_home(&["create", "feat//foo", "--no-hook"], home_path);
    assert!(
        success,
        "create failed; stdout={} stderr={}",
        stdout, stderr
    );
    // `feat//foo` should collapse to `feat_foo`, not `feat__foo`.
    assert!(
        stdout.contains("feat_foo") && !stdout.contains("feat__foo"),
        "expected collapsed sanitized name 'feat_foo', got: {}",
        stdout
    );

    let (ok, ls_out, _) = run_wtp_with_home(&["ls", "--short"], home_path);
    assert!(ok);
    assert!(
        ls_out.contains("feat_foo"),
        "ls did not find feat_foo, got: {}",
        ls_out
    );

    let _ = run_wtp_with_home(&["rm", "feat_foo", "--force"], home_path);
}

/// Seed a workspace's `.wtp/worktree.toml` with hosted repos so `ls --grep`
/// can be exercised without needing real git repositories on disk (the
/// non-`--long` path only reads the toml, it runs no git commands).
fn seed_worktrees(home: &std::path::Path, workspace: &str, repos: &[(&str, &str)]) {
    let wt_dir = home
        .join(".wtp")
        .join("workspaces")
        .join(workspace)
        .join(".wtp");
    std::fs::create_dir_all(&wt_dir).unwrap();
    let mut toml = String::from("version = \"1\"\n");
    for (i, (host, path)) in repos.iter().enumerate() {
        let slug = path.rsplit('/').next().unwrap_or(path);
        toml.push_str(&format!(
            "\n[[worktrees]]\nid = \"00000000-0000-0000-0000-0000000000{:02}\"\n\
             branch = \"main\"\nworktree_path = \"{}\"\n\
             created_at = \"2026-01-01T00:00:00+00:00\"\n\
             [worktrees.repo.hosted]\nhost = \"{}\"\npath = \"{}\"\n",
            i, slug, host, path
        ));
    }
    std::fs::write(wt_dir.join("worktree.toml"), toml).unwrap();
}

#[test]
fn test_ls_grep_filters_by_repo_name() {
    let temp_home = setup_test_env();
    let home = temp_home.path();

    let _ = run_wtp_with_home(&["create", "ws-i18n", "--no-hook"], home);
    let _ = run_wtp_with_home(&["create", "ws-pay", "--no-hook"], home);
    seed_worktrees(home, "ws-i18n", &[("byted", "oec/i18n_sdk")]);
    seed_worktrees(home, "ws-pay", &[("byted", "oec/payments")]);

    // Substring match keeps only the matching workspace.
    let (ok, out, err) = run_wtp_with_home(&["ls", "--short", "--grep", "i18n"], home);
    assert!(ok, "ls failed: {} {}", out, err);
    assert!(out.contains("ws-i18n"), "expected ws-i18n, got: {}", out);
    assert!(
        !out.contains("ws-pay"),
        "ws-pay should be filtered, got: {}",
        out
    );

    // Case-insensitive.
    let (_, out_ci, _) = run_wtp_with_home(&["ls", "--short", "--grep", "I18N"], home);
    assert!(
        out_ci.contains("ws-i18n"),
        "case-insensitive failed, got: {}",
        out_ci
    );

    // A different pattern selects the other workspace.
    let (_, out_pay, _) = run_wtp_with_home(&["ls", "--short", "--grep", "payments"], home);
    assert!(out_pay.contains("ws-pay") && !out_pay.contains("ws-i18n"));

    let _ = run_wtp_with_home(&["rm", "ws-i18n", "--force"], home);
    let _ = run_wtp_with_home(&["rm", "ws-pay", "--force"], home);
}

#[test]
fn test_ls_grep_no_match_reports_clearly() {
    let temp_home = setup_test_env();
    let home = temp_home.path();

    let _ = run_wtp_with_home(&["create", "ws-only", "--no-hook"], home);
    seed_worktrees(home, "ws-only", &[("byted", "oec/i18n_sdk")]);

    // Default (non-short) mode prints a clear message and exits 0.
    let (ok, out, err) = run_wtp_with_home(&["ls", "--grep", "zzz_nomatch"], home);
    assert!(ok, "ls failed: {} {}", out, err);
    assert!(
        out.contains("No workspaces contain a repo matching"),
        "expected no-match message, got: {}",
        out
    );
    assert!(
        !out.contains("ws-only"),
        "ws-only should not appear, got: {}",
        out
    );

    // Short mode prints nothing on no match (script-friendly).
    let (ok2, out2, _) = run_wtp_with_home(&["ls", "--short", "--grep", "zzz_nomatch"], home);
    assert!(ok2);
    assert!(
        out2.trim().is_empty(),
        "short no-match should be empty, got: {}",
        out2
    );

    let _ = run_wtp_with_home(&["rm", "ws-only", "--force"], home);
}

#[test]
fn test_ls_grep_empty_pattern_is_match_all() {
    let temp_home = setup_test_env();
    let home = temp_home.path();

    // One workspace with a repo, one with none.
    let _ = run_wtp_with_home(&["create", "ws-has", "--no-hook"], home);
    let _ = run_wtp_with_home(&["create", "ws-empty", "--no-hook"], home);
    seed_worktrees(home, "ws-has", &[("byted", "oec/i18n_sdk")]);

    // An empty pattern behaves like plain `ls` (grep match-all semantics):
    // it must NOT silently drop the repo-less workspace.
    let (ok, out, err) = run_wtp_with_home(&["ls", "--short", "--grep", ""], home);
    assert!(ok, "ls failed: {} {}", out, err);
    assert!(
        out.contains("ws-has") && out.contains("ws-empty"),
        "empty pattern should list all workspaces, got: {}",
        out
    );

    let _ = run_wtp_with_home(&["rm", "ws-has", "--force"], home);
    let _ = run_wtp_with_home(&["rm", "ws-empty", "--force"], home);
}

#[test]
fn test_create_workspace_rejects_pathological_name() {
    let temp_home = setup_test_env();
    let home_path = temp_home.path();

    // A name consisting only of separators sanitizes to an empty string
    // and must be rejected so we don't create a misnamed workspace.
    let (success, stdout, stderr) = run_wtp_with_home(&["create", "///", "--no-hook"], home_path);
    assert!(
        !success,
        "create unexpectedly succeeded for '///'; stdout={} stderr={}",
        stdout, stderr
    );
    assert!(
        stderr.contains("empty") || stderr.contains("reserved") || stderr.contains("sanitiz"),
        "expected rejection message about sanitization, got stderr: {}",
        stderr
    );
}
