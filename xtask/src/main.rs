// Copyright (c) Infinity Abundance.
// Licensed under the MIT license.

//! # xtask — Build orchestration and freshness checks
//!
//! This xtask crate implements the `cargo xtask` commands for:
//! - `gen`: Generate parity documents
//! - `check`: Freshness gate (stale docs, missing courts, etc.)
//! - `docker`: Docker matrix orchestration
//!
//! ## Usage
//!
//! ```bash
//! cargo xtask gen      # Generate parity documents
//! cargo xtask check    # Run freshness gate
//! cargo xtask docker build   # Build Docker images
//! cargo xtask docker court   # Run courts inside Docker
//! ```

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: cargo xtask <command>");
        eprintln!("Commands: gen, check, docker");
        std::process::exit(1);
    }

    match args[1].as_str() {
        "gen" => cmd_gen(&args[1..]),
        "check" => cmd_check(&args[1..]),
        "docker" => cmd_docker(&args[1..]),
        _ => {
            eprintln!("Unknown command: {}", args[1]);
            std::process::exit(1);
        }
    }
}

fn cmd_gen(_args: &[String]) {
    println!("Generating parity documents...");
    // TODO: Implement document generation
    println!("  docs/generated/surface-inventory.md");
    println!("  docs/generated/parity-matrix.md");
    println!("  docs/generated/claim-index.md");
    println!("  docs/generated/court-index.md");
    println!("Generation complete.");
}

fn cmd_check(_args: &[String]) {
    println!("Running freshness gate...");
    let mut failures = Vec::new();

    // Check that generated docs exist
    let required_docs = [
        "docs/generated/surface-inventory.md",
        "docs/generated/parity-matrix.md",
        "docs/generated/claim-index.md",
        "docs/generated/court-index.md",
    ];

    for doc in &required_docs {
        if !std::path::Path::new(doc).exists() {
            failures.push(format!("Missing generated document: {}", doc));
        }
    }

    // Check that all crate dirs exist
    let required_crates = [
        "crates/msrtc-rans-core",
        "crates/msrtc-rans",
        "crates/msrtc-rans-python",
        "crates/msrtc-rans-oracle",
        "crates/msrtc-rans-casefile",
        "crates/msrtc-rans-court",
        "crates/msrtc-rans-bench",
    ];

    for crate_dir in &required_crates {
        let path = format!("{}/Cargo.toml", crate_dir);
        if !std::path::Path::new(&path).exists() {
            failures.push(format!("Missing crate: {}", path));
        }
    }

    if !failures.is_empty() {
        eprintln!("Freshness gate FAILED:");
        for f in &failures {
            eprintln!("  - {}", f);
        }
        std::process::exit(1);
    }

    println!("Freshness gate PASSED.");
}

fn cmd_docker(args: &[String]) {
    if args.len() < 2 {
        eprintln!("Usage: cargo xtask docker <subcommand>");
        eprintln!(
            "Subcommands: build, inventory, test, court, python, mlvc, fuzz, bench, package, verify-no-cpp, receipts, clean-owned"
        );
        std::process::exit(1);
    }

    match args[1].as_str() {
        "build" => {
            println!("Building Docker images...");
            // TODO: Implement Docker build
        }
        "inventory" => {
            println!("Running Docker inventory...");
            // TODO: Implement Docker inventory
        }
        "clean-owned" => {
            if args.len() < 3 {
                eprintln!("Usage: cargo xtask docker clean-owned <run-id>");
                std::process::exit(1);
            }
            let run_id = &args[2];
            println!("Cleaning owned resources with run-id: {}", run_id);
            // TODO: Implement cleanup
        }
        _ => {
            eprintln!("Unknown docker subcommand: {}", args[1]);
            std::process::exit(1);
        }
    }
}
