// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com

//! Seal script — runs MSRTC.RAW.ENCODER.DIFFERENTIAL and produces receipt, transcript, manifest.
//!
//! Usage: cargo run --bin seal

use msrtc_rans_court::Court;

fn main() {
    let court = msrtc_rans_court::raw_encoder_differential::RawEncoderDifferentialCourt;

    println!("Running {}...", court.id());
    let result = court.run();
    println!(
        "Cases: {} passed, {} residuals, {} skipped out of {}",
        result.pass_count, result.residual_count, result.skipped_count, result.case_count
    );

    match msrtc_rans_court::seal::seal(&result) {
        Ok(receipt) => {
            println!("Sealed:");
            let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../courts")
                .canonicalize()
                .unwrap_or_else(|_| std::path::PathBuf::from("courts"));
            let court_slug = result.court_id.replace('.', "_");
            let stem = format!("MSRTC_{}_{}", court_slug, receipt.run_id);
            println!("  Receipt:   {}/receipts/{}.json", base.display(), stem);
            println!("  Transcript: {}/transcripts/{}.txt", base.display(), stem);
            println!("  Manifest:  {}/manifests/{}.json", base.display(), stem);
        }
        Err(e) => {
            eprintln!("Seal failed: {}", e);
            std::process::exit(1);
        }
    }

    if result.is_sealable() {
        println!("\n✅ {} is SEALED.", court.id());
        std::process::exit(0);
    } else {
        eprintln!("\n❌ {} is NOT sealable.", court.id());
        std::process::exit(1);
    }
}
