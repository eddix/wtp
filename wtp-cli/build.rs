//! Embed build metadata (build time + git commit) into the binary.
//!
//! The values are exposed to the crate as `WTP_BUILD_TIME` and
//! `WTP_BUILD_COMMIT` compile-time env vars, consumed by the CLI's
//! version string. Both local builds (INSTALL.md: `cargo install --path
//! wtp-cli` / `cargo build --release`) and the GitHub Actions release
//! workflow run inside a git checkout, so `git` is the default source;
//! `WTP_BUILD_COMMIT` can override it for environments without git.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=WTP_BUILD_COMMIT");
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    // Re-run when HEAD moves (new commit, branch switch) so the embedded
    // commit hash stays accurate across incremental builds.
    if let Some(git_dir) = git_dir() {
        println!("cargo:rerun-if-changed={}", git_dir.join("HEAD").display());
        if let Some(ref_file) = head_ref_file(&git_dir) {
            println!("cargo:rerun-if-changed={}", ref_file.display());
        }
    }

    let commit = std::env::var("WTP_BUILD_COMMIT")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(git_short_commit)
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=WTP_BUILD_TIME={}", build_time());
    println!("cargo:rustc-env=WTP_BUILD_COMMIT={}", commit);
}

/// Build timestamp as `YYYY-MM-DD_HH:MM:SS`.
///
/// Honors `SOURCE_DATE_EPOCH` (reproducible-builds convention, rendered as
/// UTC); otherwise uses the local time of the build machine.
fn build_time() -> String {
    const FMT: &str = "%Y-%m-%d_%H:%M:%S";
    if let Some(epoch) = std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .and_then(|secs| chrono::DateTime::from_timestamp(secs, 0))
    {
        return epoch.format(FMT).to_string();
    }
    chrono::Local::now().format(FMT).to_string()
}

fn run_git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!s.is_empty()).then_some(s)
}

fn git_short_commit() -> Option<String> {
    run_git(&["rev-parse", "--short", "HEAD"])
}

fn git_dir() -> Option<PathBuf> {
    run_git(&["rev-parse", "--absolute-git-dir"]).map(PathBuf::from)
}

/// The ref file HEAD points to (e.g. `.git/refs/heads/main`), so that new
/// commits on the same branch also trigger a re-run.
fn head_ref_file(git_dir: &std::path::Path) -> Option<PathBuf> {
    let head = run_git(&["symbolic-ref", "-q", "HEAD"])?;
    let path = git_dir.join(head);
    path.exists().then_some(path)
}
