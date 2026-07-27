// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com

//! Build script for msrtc-rans-court.
//!
//! Captures the Git commit SHA and dirty-tree status at build time
//! for inclusion in court receipts and residuals.

use std::process::Command;

fn main() {
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

    // Rerun if the git HEAD changes
    println!("cargo:rerun-if-changed=../../.git/HEAD");
}
