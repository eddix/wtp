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
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
    // Build metadata embedded by build.rs, e.g.
    // "wtp 0.1.0 (built at 2026-06-12_15:23:51, commit 8639996)"
    assert!(
        stdout.contains("built at ") && stdout.contains("commit "),
        "expected build metadata in version, got: {}",
        stdout
    );
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
    let _ = run_wtp_with_home(&["remove", "test-short", "--force"], temp_home.path());
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
    let _ = run_wtp_with_home(&["remove", "test-hook-ws", "--force"], home_path);
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
    let _ = run_wtp_with_home(&["remove", "test-no-hook-ws", "--force"], home_path);
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
        &["remove", "hotfix_update_task_issue_1234", "--force"],
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

    let _ = run_wtp_with_home(&["remove", "feat_foo", "--force"], home_path);
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

    let _ = run_wtp_with_home(&["remove", "ws-i18n", "--force"], home);
    let _ = run_wtp_with_home(&["remove", "ws-pay", "--force"], home);
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

    let _ = run_wtp_with_home(&["remove", "ws-only", "--force"], home);
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

    let _ = run_wtp_with_home(&["remove", "ws-has", "--force"], home);
    let _ = run_wtp_with_home(&["remove", "ws-empty", "--force"], home);
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

/// Create a real git repository with an initial commit on `main`.
fn init_git_repo(dir: &std::path::Path) {
    let run = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .expect("failed to run git");
        assert!(
            out.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
    };
    run(&["init", "-b", "main"]);
    run(&["config", "user.email", "test@example.com"]);
    run(&["config", "user.name", "Test"]);
    std::fs::write(dir.join("README.md"), "hello").unwrap();
    run(&["add", "."]);
    run(&["commit", "-m", "init"]);
}

#[test]
fn test_import_same_repo_multiple_branches() {
    let temp_home = setup_test_env();
    let home = temp_home.path();

    let repo_dir = TempDir::new().unwrap();
    let repo_path = repo_dir.path();
    init_git_repo(repo_path);
    let repo_str = repo_path.to_str().unwrap();
    let slug = repo_path.file_name().unwrap().to_str().unwrap();

    let (ok, out, err) = run_wtp_with_home(&["create", "ws-multi", "--no-hook"], home);
    assert!(ok, "create failed: {} {}", out, err);
    let ws_path = home.join(".wtp").join("workspaces").join("ws-multi");

    // First import: directory named after the repo slug, as before.
    let (ok, out, err) = run_wtp_in_dir_with_home(
        &["import", "--repo", repo_str, "-b", "release-area-a-dev"],
        Some(&ws_path),
        home,
    );
    assert!(ok, "first import failed: {} {}", out, err);
    assert!(ws_path.join(slug).is_dir());

    // Same repo again without --with-branch-name: clear error with a hint.
    let (ok, out, err) = run_wtp_in_dir_with_home(
        &["import", "--repo", repo_str, "-b", "release-area-b-dev"],
        Some(&ws_path),
        home,
    );
    assert!(!ok, "unflagged second import should fail");
    let combined = format!("{} {}", out, err);
    assert!(
        combined.contains("--with-branch-name"),
        "expected --with-branch-name hint, got: {}",
        combined
    );

    // Same repo + same branch even with the flag: rejected.
    let (ok, out, err) = run_wtp_in_dir_with_home(
        &[
            "import",
            "--repo",
            repo_str,
            "-b",
            "release-area-a-dev",
            "--with-branch-name",
        ],
        Some(&ws_path),
        home,
    );
    assert!(!ok, "duplicate branch import should fail");
    let combined = format!("{} {}", out, err);
    assert!(
        combined.contains("already has a worktree for branch"),
        "expected duplicate-branch error, got: {}",
        combined
    );

    // Same repo, different branch, with the flag: directory is slug@branch.
    let (ok, out, err) = run_wtp_in_dir_with_home(
        &[
            "import",
            "--repo",
            repo_str,
            "-b",
            "release-area-b-dev",
            "--with-branch-name",
        ],
        Some(&ws_path),
        home,
    );
    assert!(ok, "flagged import failed: {} {}", out, err);
    let branch_dir = format!("{}@release-area-b-dev", slug);
    assert!(ws_path.join(&branch_dir).is_dir());

    // Status lists both worktrees of the same repo.
    let (ok, out, err) = run_wtp_in_dir_with_home(&["status"], Some(&ws_path), home);
    assert!(ok, "status failed: {} {}", out, err);
    assert!(
        out.contains("release-area-a-dev") && out.contains("release-area-b-dev"),
        "status should list both branches, got: {}",
        out
    );

    // Eject by directory name removes only that worktree.
    let (ok, out, err) = run_wtp_in_dir_with_home(&["eject", &branch_dir], Some(&ws_path), home);
    assert!(ok, "eject failed: {} {}", out, err);
    assert!(!ws_path.join(&branch_dir).exists());
    assert!(ws_path.join(slug).is_dir());

    // Re-import branch b so the workspace again holds two worktrees of the
    // same repo, then remove the whole workspace (exercises batch cleanup).
    let (ok, out, err) = run_wtp_in_dir_with_home(
        &[
            "import",
            "--repo",
            repo_str,
            "-b",
            "release-area-b-dev",
            "--with-branch-name",
        ],
        Some(&ws_path),
        home,
    );
    assert!(ok, "re-import failed: {} {}", out, err);

    let (ok, out, err) = run_wtp_with_home(&["remove", "ws-multi", "--force"], home);
    assert!(ok, "rm failed: {} {}", out, err);
    assert!(!ws_path.exists());
}

#[test]
fn test_import_parent_creates_stacked_layer() {
    let temp_home = setup_test_env();
    let home = temp_home.path();

    let repo_dir = TempDir::new().unwrap();
    let repo_path = repo_dir.path();
    init_git_repo(repo_path);
    let repo_str = repo_path.to_str().unwrap();
    let slug = repo_path.file_name().unwrap().to_str().unwrap();

    let (ok, out, err) = run_wtp_with_home(&["create", "ws-stack", "--no-hook"], home);
    assert!(ok, "create failed: {} {}", out, err);
    let ws_path = home.join(".wtp").join("workspaces").join("ws-stack");

    // Bottom layer: a plain import, no parent.
    let (ok, out, err) = run_wtp_in_dir_with_home(
        &["import", "--repo", repo_str, "-b", "feat-1"],
        Some(&ws_path),
        home,
    );
    assert!(ok, "bottom import failed: {} {}", out, err);
    let feat1_dir = ws_path.join(slug);
    assert!(feat1_dir.is_dir());

    // --parent conflicts with --base at the CLI level.
    let (ok, _, err) = run_wtp_in_dir_with_home(
        &[
            "import", "--repo", repo_str, "-b", "feat-2", "--parent", "feat-1", "-B", "main",
        ],
        Some(&ws_path),
        home,
    );
    assert!(!ok, "--parent with --base should be rejected");
    assert!(
        err.contains("cannot be used with"),
        "expected clap conflict error, got: {}",
        err
    );

    // --parent with an unknown ref fails before creating anything.
    let (ok, out, err) = run_wtp_in_dir_with_home(
        &[
            "import",
            "--repo",
            repo_str,
            "-b",
            "feat-2",
            "--parent",
            "no-such-ref",
        ],
        Some(&ws_path),
        home,
    );
    assert!(!ok, "unknown parent ref should fail");
    let combined = format!("{} {}", out, err);
    assert!(
        combined.contains("not found"),
        "expected parent-not-found error, got: {}",
        combined
    );

    // Stack a layer from inside the parent's worktree directory: PATH is
    // omitted and the repo is inferred; --with-branch-name is implied.
    let (ok, out, err) = run_wtp_in_dir_with_home(
        &["import", "-b", "feat-2", "--parent", "feat-1"],
        Some(&feat1_dir),
        home,
    );
    assert!(ok, "stacked import failed: {} {}", out, err);
    assert!(out.contains("Stacked on parent"), "got: {}", out);
    let feat2_dir = ws_path.join(format!("{}@feat-2", slug));
    assert!(feat2_dir.is_dir(), "implied --with-branch-name directory");

    // The stack edge and fork point are recorded in worktree.toml.
    let toml_text = std::fs::read_to_string(ws_path.join(".wtp").join("worktree.toml")).unwrap();
    assert!(
        toml_text.contains("parent = \"feat-1\""),
        "worktree.toml missing parent: {}",
        toml_text
    );
    assert!(
        toml_text.contains("parent_head = \""),
        "worktree.toml missing parent_head: {}",
        toml_text
    );

    // --parent without PATH from the workspace root (not a worktree dir)
    // errors instead of falling through to the interactive picker.
    let (ok, out, err) = run_wtp_in_dir_with_home(
        &["import", "-b", "feat-3", "--parent", "feat-2"],
        Some(&ws_path),
        home,
    );
    assert!(!ok, "--parent without PATH at workspace root should fail");
    let combined = format!("{} {}", out, err);
    assert!(
        combined.contains("worktree directory"),
        "expected inference error, got: {}",
        combined
    );

    // Add a commit on feat-2 so the divergence marker has something to show.
    let run_git = |dir: &std::path::Path, args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
    };
    std::fs::write(feat2_dir.join("layer2.txt"), "l2").unwrap();
    run_git(&feat2_dir, &["add", "."]);
    run_git(&feat2_dir, &["commit", "-m", "layer 2 work"]);

    // Compact status shows the tree marker and the ahead count.
    let (ok, out, err) = run_wtp_in_dir_with_home(&["status"], Some(&ws_path), home);
    assert!(ok, "status failed: {} {}", out, err);
    assert!(
        out.contains("└ feat-2") || out.contains("└"),
        "expected tree marker in status, got: {}",
        out
    );
    assert!(
        out.contains("↑1"),
        "expected ahead marker for feat-2, got: {}",
        out
    );

    // Long status shows the Parent line.
    let (ok, out, err) = run_wtp_in_dir_with_home(&["status", "--long"], Some(&ws_path), home);
    assert!(ok, "status --long failed: {} {}", out, err);
    assert!(
        out.contains("Parent:") && out.contains("+1 ahead"),
        "expected parent divergence in long status, got: {}",
        out
    );

    // Deleting the parent branch surfaces "parent missing". The branch is
    // checked out in feat1_dir's worktree, so drop that worktree first.
    let (ok, out, err) = run_wtp_in_dir_with_home(&["eject", slug], Some(&ws_path), home);
    assert!(ok, "eject failed: {} {}", out, err);
    run_git(repo_path, &["branch", "-D", "feat-1"]);
    let (ok, out, err) = run_wtp_in_dir_with_home(&["status"], Some(&ws_path), home);
    assert!(ok, "status after branch delete failed: {} {}", out, err);
    assert!(
        out.contains("parent missing"),
        "expected parent-missing marker, got: {}",
        out
    );

    let _ = run_wtp_with_home(&["remove", "ws-stack", "--force"], home);
}

#[test]
fn test_retarget_and_eject_hint() {
    let temp_home = setup_test_env();
    let home = temp_home.path();

    let repo_dir = TempDir::new().unwrap();
    let repo_path = repo_dir.path();
    init_git_repo(repo_path);
    let repo_str = repo_path.to_str().unwrap();
    let slug = repo_path.file_name().unwrap().to_str().unwrap();

    let (ok, out, err) = run_wtp_with_home(&["create", "ws-rt", "--no-hook"], home);
    assert!(ok, "create failed: {} {}", out, err);
    let ws_path = home.join(".wtp").join("workspaces").join("ws-rt");

    let (ok, out, err) = run_wtp_in_dir_with_home(
        &["import", "--repo", repo_str, "-b", "feat-1"],
        Some(&ws_path),
        home,
    );
    assert!(ok, "bottom import failed: {} {}", out, err);
    let feat1_dir = ws_path.join(slug);
    let (ok, out, err) = run_wtp_in_dir_with_home(
        &["import", "-b", "feat-2", "--parent", "feat-1"],
        Some(&feat1_dir),
        home,
    );
    assert!(ok, "stacked import failed: {} {}", out, err);
    let feat2_dirname = format!("{}@feat-2", slug);
    let feat2_dir = ws_path.join(&feat2_dirname);
    let toml_path = ws_path.join(".wtp").join("worktree.toml");
    let fork_point_before = std::fs::read_to_string(&toml_path)
        .unwrap()
        .lines()
        .find(|l| l.starts_with("parent_head"))
        .unwrap()
        .to_string();

    // Two-argument form from the workspace root; the old fork point must
    // survive the retarget (squash-merge transplant relies on it).
    let (ok, out, err) =
        run_wtp_in_dir_with_home(&["retarget", &feat2_dirname, "main"], Some(&ws_path), home);
    assert!(ok, "retarget failed: {} {}", out, err);
    assert!(
        out.contains("wtp restack"),
        "expected restack hint, got: {}",
        out
    );
    let toml_text = std::fs::read_to_string(&toml_path).unwrap();
    assert!(
        toml_text.contains("parent = \"main\""),
        "parent not updated: {}",
        toml_text
    );
    assert!(
        toml_text.contains(&fork_point_before),
        "fork point must be preserved across retarget: {}",
        toml_text
    );

    // One-argument form from inside the worktree directory.
    let (ok, out, err) = run_wtp_in_dir_with_home(&["retarget", "feat-1"], Some(&feat2_dir), home);
    assert!(ok, "one-arg retarget failed: {} {}", out, err);
    let toml_text = std::fs::read_to_string(&toml_path).unwrap();
    assert!(toml_text.contains("parent = \"feat-1\""), "{}", toml_text);

    // Cycle: feat-1 cannot be reparented onto its own descendant.
    let (ok, out, err) =
        run_wtp_in_dir_with_home(&["retarget", slug, "feat-2"], Some(&ws_path), home);
    assert!(!ok, "cycle retarget should fail");
    let combined = format!("{} {}", out, err);
    assert!(
        combined.contains("cycle"),
        "expected cycle error: {}",
        combined
    );

    // Self-parent and unknown refs are rejected.
    let (ok, _, _) = run_wtp_in_dir_with_home(&["retarget", "feat-2"], Some(&feat2_dir), home);
    assert!(!ok, "self-parent should fail");
    let (ok, out, err) =
        run_wtp_in_dir_with_home(&["retarget", "no-such-ref"], Some(&feat2_dir), home);
    assert!(!ok, "unknown ref should fail");
    let combined = format!("{} {}", out, err);
    assert!(
        combined.contains("not found"),
        "expected not-found error: {}",
        combined
    );

    // A flat worktree gaining a parent for the first time gets a fork point.
    let (ok, out, err) =
        run_wtp_in_dir_with_home(&["retarget", slug, "main"], Some(&ws_path), home);
    assert!(ok, "flat retarget failed: {} {}", out, err);
    let toml_text = std::fs::read_to_string(&toml_path).unwrap();
    assert_eq!(
        toml_text.matches("parent_head = \"").count(),
        2,
        "flat worktree should gain a fork point: {}",
        toml_text
    );

    // Ejecting a layer that has stack children prints a reparent hint.
    let (ok, out, err) = run_wtp_in_dir_with_home(&["eject", slug], Some(&ws_path), home);
    assert!(ok, "eject failed: {} {}", out, err);
    let combined = format!("{} {}", out, err);
    assert!(
        combined.contains("wtp retarget") && combined.contains("feat-2"),
        "expected stack-children hint on eject, got: {}",
        combined
    );

    let _ = run_wtp_with_home(&["remove", "ws-rt", "--force"], home);
}

fn run_git(dir: &std::path::Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
}

fn git_log_subjects(dir: &std::path::Path) -> String {
    let out = Command::new("git")
        .args(["log", "--format=%s"])
        .current_dir(dir)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// Build ws with feat-1 <- feat-2 <- feat-3 and one commit on each layer.
/// Returns (ws_path, slug).
fn setup_three_layer_stack(
    home: &std::path::Path,
    repo_path: &std::path::Path,
    ws_name: &str,
) -> (PathBuf, String) {
    init_git_repo(repo_path);
    let repo_str = repo_path.to_str().unwrap();
    let slug = repo_path.file_name().unwrap().to_str().unwrap().to_string();

    let (ok, out, err) = run_wtp_with_home(&["create", ws_name, "--no-hook"], home);
    assert!(ok, "create failed: {} {}", out, err);
    let ws_path = home.join(".wtp").join("workspaces").join(ws_name);

    let (ok, out, err) = run_wtp_in_dir_with_home(
        &["import", "--repo", repo_str, "-b", "feat-1"],
        Some(&ws_path),
        home,
    );
    assert!(ok, "feat-1 import failed: {} {}", out, err);
    let feat1_dir = ws_path.join(&slug);

    for (branch, parent, file) in [("feat-2", "feat-1", "two"), ("feat-3", "feat-2", "three")] {
        let parent_dir = if parent == "feat-1" {
            feat1_dir.clone()
        } else {
            ws_path.join(format!("{}@{}", slug, parent))
        };
        let (ok, out, err) = run_wtp_in_dir_with_home(
            &["import", "-b", branch, "--parent", parent],
            Some(&parent_dir),
            home,
        );
        assert!(ok, "{} import failed: {} {}", branch, out, err);
        let layer_dir = ws_path.join(format!("{}@{}", slug, branch));
        std::fs::write(layer_dir.join(format!("{}.txt", file)), file).unwrap();
        run_git(&layer_dir, &["add", "."]);
        run_git(&layer_dir, &["commit", "-m", &format!("{} work", branch)]);
    }

    (ws_path, slug)
}

#[test]
fn test_restack_cascade_and_idempotent() {
    let temp_home = setup_test_env();
    let home = temp_home.path();
    let repo_dir = TempDir::new().unwrap();
    let (ws_path, slug) = setup_three_layer_stack(home, repo_dir.path(), "ws-cascade");
    let feat1_dir = ws_path.join(&slug);
    let feat3_dir = ws_path.join(format!("{}@feat-3", slug));

    // Advance feat-1 so both children fall behind.
    std::fs::write(feat1_dir.join("base.txt"), "base work").unwrap();
    run_git(&feat1_dir, &["add", "."]);
    run_git(&feat1_dir, &["commit", "-m", "feat-1 advance"]);

    // Restack everything from the workspace root.
    let (ok, out, err) = run_wtp_in_dir_with_home(&["restack"], Some(&ws_path), home);
    assert!(ok, "restack failed: {} {}", out, err);
    assert!(
        out.contains("2 rebased"),
        "expected two layers rebased, got: {}",
        out
    );
    assert!(
        out.contains("--force-with-lease") && out.contains("feat-2") && out.contains("feat-3"),
        "expected force-push checklist, got: {}",
        out
    );

    // The advance commit propagated to the top of the stack.
    let log = git_log_subjects(&feat3_dir);
    assert!(
        log.contains("feat-1 advance")
            && log.contains("feat-2 work")
            && log.contains("feat-3 work"),
        "feat-3 history after restack: {}",
        log
    );

    // Idempotent: a second run skips every layer.
    let (ok, out, err) = run_wtp_in_dir_with_home(&["restack"], Some(&ws_path), home);
    assert!(ok, "second restack failed: {} {}", out, err);
    assert!(
        out.contains("0 rebased") && out.contains("2 already up to date"),
        "expected all-skip on second run, got: {}",
        out
    );

    // Scoped run from inside a layer directory also succeeds (same chain).
    let (ok, out, err) = run_wtp_in_dir_with_home(&["restack"], Some(&feat3_dir), home);
    assert!(ok, "scoped restack failed: {} {}", out, err);

    let _ = run_wtp_with_home(&["remove", "ws-cascade", "--force"], home);
}

#[test]
fn test_restack_dirty_preflight_blocks() {
    let temp_home = setup_test_env();
    let home = temp_home.path();
    let repo_dir = TempDir::new().unwrap();
    let (ws_path, slug) = setup_three_layer_stack(home, repo_dir.path(), "ws-dirty");
    let feat2_dir = ws_path.join(format!("{}@feat-2", slug));

    std::fs::write(feat2_dir.join("wip.txt"), "uncommitted").unwrap();

    let (ok, out, err) = run_wtp_in_dir_with_home(&["restack"], Some(&ws_path), home);
    assert!(!ok, "restack should refuse dirty layers");
    let combined = format!("{} {}", out, err);
    assert!(
        combined.contains("uncommitted changes") && combined.contains("feat-2"),
        "expected dirty preflight error, got: {}",
        combined
    );

    let _ = run_wtp_with_home(&["remove", "ws-dirty", "--force"], home);
}

#[test]
fn test_restack_conflict_stops_then_resumes() {
    let temp_home = setup_test_env();
    let home = temp_home.path();
    let repo_dir = TempDir::new().unwrap();
    let (ws_path, slug) = setup_three_layer_stack(home, repo_dir.path(), "ws-conflict");
    let feat1_dir = ws_path.join(&slug);
    let feat2_dir = ws_path.join(format!("{}@feat-2", slug));

    // feat-1 rewrites the same file feat-2 created -> guaranteed conflict.
    std::fs::write(feat1_dir.join("two.txt"), "conflicting base content").unwrap();
    run_git(&feat1_dir, &["add", "."]);
    run_git(&feat1_dir, &["commit", "-m", "feat-1 conflicting change"]);

    let (ok, out, err) = run_wtp_in_dir_with_home(&["restack"], Some(&ws_path), home);
    assert!(!ok, "restack should stop on conflict");
    let combined = format!("{} {}", out, err);
    assert!(
        combined.contains("Conflict while restacking")
            && combined.contains("two.txt")
            && combined.contains(&feat2_dir.display().to_string())
            && combined.contains("git rebase --continue"),
        "expected structured conflict report, got: {}",
        combined
    );

    // Resolve like an agent would: fix the file, continue the rebase.
    std::fs::write(feat2_dir.join("two.txt"), "merged content").unwrap();
    run_git(&feat2_dir, &["add", "two.txt"]);
    let out = Command::new("git")
        .args(["rebase", "--continue"])
        .env("GIT_EDITOR", "true")
        .current_dir(&feat2_dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "rebase --continue failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Re-run: feat-2 is detected as done (fork point healed), feat-3 rebases.
    let (ok, out, err) = run_wtp_in_dir_with_home(&["restack"], Some(&ws_path), home);
    assert!(ok, "resumed restack failed: {} {}", out, err);
    assert!(
        out.contains("1 rebased") && out.contains("1 already up to date"),
        "expected feat-2 skipped and feat-3 rebased, got: {}",
        out
    );

    let _ = run_wtp_with_home(&["remove", "ws-conflict", "--force"], home);
}

#[test]
fn test_restack_after_squash_merge_via_fork_point() {
    let temp_home = setup_test_env();
    let home = temp_home.path();

    let repo_dir = TempDir::new().unwrap();
    let repo_path = repo_dir.path();
    init_git_repo(repo_path);
    let repo_str = repo_path.to_str().unwrap();
    let slug = repo_path.file_name().unwrap().to_str().unwrap();

    let (ok, out, err) = run_wtp_with_home(&["create", "ws-squash", "--no-hook"], home);
    assert!(ok, "create failed: {} {}", out, err);
    let ws_path = home.join(".wtp").join("workspaces").join("ws-squash");

    // feat-1 with two commits, feat-2 stacked on it with one commit.
    let (ok, out, err) = run_wtp_in_dir_with_home(
        &["import", "--repo", repo_str, "-b", "feat-1"],
        Some(&ws_path),
        home,
    );
    assert!(ok, "feat-1 import failed: {} {}", out, err);
    let feat1_dir = ws_path.join(slug);
    for n in ["one", "two"] {
        std::fs::write(feat1_dir.join(format!("{}.txt", n)), n).unwrap();
        run_git(&feat1_dir, &["add", "."]);
        run_git(&feat1_dir, &["commit", "-m", &format!("feat-1 {}", n)]);
    }
    let (ok, out, err) = run_wtp_in_dir_with_home(
        &["import", "-b", "feat-2", "--parent", "feat-1"],
        Some(&feat1_dir),
        home,
    );
    assert!(ok, "feat-2 import failed: {} {}", out, err);
    let feat2_dirname = format!("{}@feat-2", slug);
    let feat2_dir = ws_path.join(&feat2_dirname);
    std::fs::write(feat2_dir.join("layer2.txt"), "l2").unwrap();
    run_git(&feat2_dir, &["add", "."]);
    run_git(&feat2_dir, &["commit", "-m", "feat-2 work"]);

    // Squash-merge feat-1 into main (in the original repo), then drop the
    // feat-1 layer and branch — the standard "bottom PR landed" flow.
    run_git(repo_path, &["checkout", "main"]);
    run_git(repo_path, &["merge", "--squash", "feat-1"]);
    run_git(repo_path, &["commit", "-m", "feat-1 squashed into main"]);
    let (ok, out, err) = run_wtp_in_dir_with_home(&["eject", slug], Some(&ws_path), home);
    assert!(ok, "eject feat-1 failed: {} {}", out, err);
    run_git(repo_path, &["branch", "-D", "feat-1"]);

    // Reparent feat-2 onto main and restack: the recorded fork point means
    // only feat-2's own commit is replayed, so no conflict despite the
    // squash (patch-ids of feat-1's commits don't match the squash commit).
    let (ok, out, err) =
        run_wtp_in_dir_with_home(&["retarget", &feat2_dirname, "main"], Some(&ws_path), home);
    assert!(ok, "retarget failed: {} {}", out, err);
    let (ok, out, err) = run_wtp_in_dir_with_home(&["restack"], Some(&feat2_dir), home);
    assert!(ok, "squash restack should be clean: {} {}", out, err);
    assert!(out.contains("1 rebased"), "got: {}", out);

    // feat-2 sits on the squash commit; feat-1's original commits are gone.
    let log = git_log_subjects(&feat2_dir);
    assert!(
        log.contains("feat-2 work") && log.contains("feat-1 squashed into main"),
        "feat-2 should sit on the squash commit: {}",
        log
    );
    assert!(
        !log.contains("feat-1 one") && !log.contains("feat-1 two"),
        "feat-1's pre-squash commits must not be replayed: {}",
        log
    );

    let _ = run_wtp_with_home(&["remove", "ws-squash", "--force"], home);
}

#[test]
fn test_rm_is_alias_for_remove() {
    let temp_home = setup_test_env();
    let home = temp_home.path();

    let (ok, out, err) = run_wtp_with_home(&["create", "ws-rm-alias", "--no-hook"], home);
    assert!(ok, "create failed: {} {}", out, err);
    let ws_path = home.join(".wtp").join("workspaces").join("ws-rm-alias");
    assert!(ws_path.is_dir());

    let (ok, out, err) = run_wtp_with_home(&["rm", "ws-rm-alias", "--force"], home);
    assert!(ok, "rm alias failed: {} {}", out, err);
    assert!(!ws_path.exists());
}
