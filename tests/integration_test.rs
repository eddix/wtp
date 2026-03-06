//! Integration tests for wtp

use std::process::Command;
use std::path::PathBuf;
use tempfile::TempDir;
use std::sync::Mutex;

// Use a mutex to ensure tests don't run in parallel and interfere with each other
static TEST_MUTEX: Mutex<()> = Mutex::new(());

fn wtp_bin() -> PathBuf {
    // Find the compiled binary
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join("target/release/wtp")
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
    let _guard = TEST_MUTEX.lock().unwrap();
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
    assert!(stdout.contains("Workspace Management"));
    assert!(stdout.contains("Repository Operations"));
    assert!(stdout.contains("Utilities"));
    assert!(!stdout.contains("  init  ")); // init command was removed (but shell-init exists)
}

#[test]
fn test_wtp_version() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let temp_home = setup_test_env();
    
    let (success, stdout, _) = run_wtp_with_home(&["--version"], temp_home.path());
    assert!(success);
    assert!(stdout.contains("0.1.0"));
}

#[test]
fn test_import_requires_workspace() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let temp_home = setup_test_env();
    let temp_dir = TempDir::new().unwrap();
    
    // Without --workspace and not in a workspace, should fail with "Not in a workspace"
    let (success, stdout, stderr) = run_wtp_in_dir_with_home(
        &["import", "some/path"],
        Some(temp_dir.path()),
        temp_home.path()
    );
    assert!(!success);
    let combined = format!("{} {}", stdout, stderr);
    assert!(combined.contains("Not in a workspace") || combined.contains("workspace"),
            "Expected workspace-related error, got: {}", combined);
}

#[test]
fn test_status_not_in_workspace() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let temp_home = setup_test_env();
    let temp_dir = TempDir::new().unwrap();
    
    // Without --workspace and not in a workspace, should fail with "Not in a workspace"
    let (success, stdout, stderr) = run_wtp_in_dir_with_home(
        &["status"],
        Some(temp_dir.path()),
        temp_home.path()
    );
    assert!(!success);
    let combined = format!("{} {}", stdout, stderr);
    assert!(combined.contains("Not in a workspace") || combined.contains("workspace"),
            "Expected workspace-related error, got: {}", combined);
}

#[test]
fn test_cd_requires_shell_integration() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let temp_home = setup_test_env();
    
    // Use a workspace name that definitely doesn't exist
    let (success, stdout, stderr) = run_wtp_with_home(&["cd", "nonexistent-workspace-xyz"], temp_home.path());
    assert!(!success);
    let combined = format!("{} {}", stdout, stderr);
    // Either "not found" or "shell integration" error is acceptable
    assert!(combined.contains("not found") || combined.contains("shell integration") || combined.contains("Shell integration"),
            "Expected error message, got: {}", combined);
}

#[test]
fn test_shell_init_outputs_wrapper() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let temp_home = setup_test_env();
    
    let (success, stdout, _) = run_wtp_with_home(&["shell-init"], temp_home.path());
    assert!(success);
    assert!(stdout.contains("wtp() {"));
    assert!(stdout.contains("WTP_DIRECTIVE_FILE"));
}

#[test]
fn test_ls_short_format() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let temp_home = setup_test_env();
    
    // First create a workspace in the isolated temp home
    let _ = run_wtp_with_home(&["create", "test-short"], temp_home.path());
    
    let (success, stdout, _) = run_wtp_with_home(&["ls", "--short"], temp_home.path());
    assert!(success);
    // Should contain our test workspace
    assert!(stdout.contains("test-short"), "Expected 'test-short' in output, got: {}", stdout);
    
    // Cleanup
    let _ = run_wtp_with_home(&["rm", "test-short", "--force"], temp_home.path());
}

#[test]
fn test_create_workspace_with_hook() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let temp_home = setup_test_env();
    let home_path = temp_home.path();
    
    // Create a hook script
    let hooks_dir = home_path.join(".wtp").join("hooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    
    let hook_script = hooks_dir.join("on-create.sh");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(&hook_script, r#"#!/bin/bash
echo "HOOK_RAN: $WTP_WORKSPACE_NAME"
touch "$WTP_WORKSPACE_PATH/hook-marker.txt"
"#).unwrap();
        let mut perms = std::fs::metadata(&hook_script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&hook_script, perms).unwrap();
    }
    #[cfg(not(unix))]
    {
        std::fs::write(&hook_script, r#"#!/bin/bash
echo "HOOK_RAN: $WTP_WORKSPACE_NAME"
touch "$WTP_WORKSPACE_PATH/hook-marker.txt"
"#).unwrap();
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
    assert!(stdout.contains("HOOK_RAN: test-hook-ws"), 
            "Expected hook output in stdout, got: {}", stdout);
    
    // Verify marker file was created by hook
    let workspace_path = home_path.join(".wtp").join("workspaces").join("test-hook-ws");
    let marker_file = workspace_path.join("hook-marker.txt");
    assert!(marker_file.exists(), "Hook marker file should exist");
    
    // Cleanup
    let _ = run_wtp_with_home(&["rm", "test-hook-ws", "--force"], home_path);
}

#[test]
fn test_create_workspace_skip_hook() {
    let _guard = TEST_MUTEX.lock().unwrap();
    let temp_home = setup_test_env();
    let home_path = temp_home.path();
    
    // Create a hook script that would fail if run
    let hooks_dir = home_path.join(".wtp").join("hooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    
    let hook_script = hooks_dir.join("on-create.sh");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(&hook_script, r#"#!/bin/bash
echo "HOOK_SHOULD_NOT_RUN"
exit 1
"#).unwrap();
        let mut perms = std::fs::metadata(&hook_script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&hook_script, perms).unwrap();
    }
    #[cfg(not(unix))]
    {
        std::fs::write(&hook_script, r#"#!/bin/bash
echo "HOOK_SHOULD_NOT_RUN"
exit 1
"#).unwrap();
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
    let (success, stdout, stderr) = run_wtp_with_home(&["create", "test-no-hook-ws", "--no-hook"], home_path);
    assert!(success, "Failed to create workspace: {}", stderr);
    
    // Check hook output was NOT shown
    assert!(!stdout.contains("HOOK_SHOULD_NOT_RUN"), 
            "Hook should not have run, but output contains hook text: {}", stdout);
    
    // Cleanup
    let _ = run_wtp_with_home(&["rm", "test-no-hook-ws", "--force"], home_path);
}

#[test]
fn test_eject_not_in_workspace() {
    let _guard = TEST_MUTEX.lock().unwrap();
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
    let _guard = TEST_MUTEX.lock().unwrap();
    let temp_home = setup_test_env();

    let (success, stdout, _) = run_wtp_with_home(&["help", "eject"], temp_home.path());
    assert!(success);
    assert!(stdout.contains("Eject"));
    assert!(stdout.contains("Usage:"));
    assert!(stdout.contains("wtp eject"));
}
