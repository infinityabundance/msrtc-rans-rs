// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com

//! Build script for msrtc-rans-court.
//!
//! Captures the Git commit SHA and dirty-tree status at build time
//! for inclusion in court receipts and residuals.

use std::path::Path;
use std::process::Command;

fn main() {
    // Resolve the workspace root from CARGO_MANIFEST_DIR (crates/msrtc-rans-court → workspace root)
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let git_dir = manifest_dir.join("../../.git");

    // Watch git HEAD
    let head_path = git_dir.join("HEAD");
    println!("cargo:rerun-if-changed={}", head_path.display());

    // Watch git index (staging area changes)
    let index_path = git_dir.join("index");
    if index_path.exists() {
        println!("cargo:rerun-if-changed={}", index_path.display());
    }

    // Parse HEAD to find the current branch and watch its ref
    if let Ok(head_content) = std::fs::read_to_string(&head_path) {
        if let Some(ref_path) = head_content
            .strip_prefix("ref: ")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            let branch_ref = git_dir.join(ref_path);
            println!("cargo:rerun-if-changed={}", branch_ref.display());
        }
    }

    // Capture full Git commit SHA
    let git_hash = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Capture dirty-tree status
    let git_dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .map(|o| o.stdout.len() > 0)
        .unwrap_or(false);

    println!("cargo:rustc-env=GIT_HASH={}", git_hash);
    println!("cargo:rustc-env=GIT_DIRTY={}", git_dirty);
}
