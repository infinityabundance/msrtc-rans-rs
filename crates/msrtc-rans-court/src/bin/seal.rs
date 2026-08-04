// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com

//! Seal script — runs a differential court and produces receipt, transcript, manifest.
//!
//! Usage: cargo run --bin seal [encoder|decoder]
//!   Default: encoder

use msrtc_rans_court::Court;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let which = args.get(1).map(|s| s.as_str()).unwrap_or("encoder");

    let court: Box<dyn Court> = match which {
        "decoder" => {
            Box::new(msrtc_rans_court::raw_decoder_differential::RawDecoderDifferentialCourt)
        }
        "entropy" => Box::new(msrtc_rans_court::entropy_differential::EntropyDifferentialCourt),
        "stream" => Box::new(msrtc_rans_court::stream_differential::StreamDifferentialCourt),
        _ => Box::new(msrtc_rans_court::raw_encoder_differential::RawEncoderDifferentialCourt),
    };

    println!("Running {}...", court.id());
    let result = court.run();
    println!(
        "Cases: {} passed, {} residuals, {} skipped out of {}",
        result.pass_count, result.residual_count, result.skipped_count, result.case_count
    );

    if !result.is_sealable() {
        eprintln!("\n❌ {} is NOT sealable.", court.id());
        std::process::exit(1);
    }

    match msrtc_rans_court::seal::seal(&result) {
        Ok(receipt) => {
            println!("Sealed:");
            println!("  Rust commit: {}", receipt.rust_commit);
            println!("  Run ID: {}", receipt.run_id);
            println!("✅ {} is SEALED.", court.id());
        }
        Err(e) => {
            eprintln!("Seal failed: {}", e);
            std::process::exit(1);
        }
    }
}
