// Licensed under the MIT license.
// Author: Riaan de Beer - github.com/infinityabundance - rdebeer.infinityabundance@gmail.com

//! # msrtc-rans-bench
//!
//! Deterministic benchmark harness for the msrtc_rans entropy coder.
//!
//! Measures throughput (symbols/sec and MB/s) for:
//! - raw RansByte / Rans64 encode and decode
//! - entropy encode / decode (both variants, with bypass)
//! - persistent stream multipart encode / decode
//!
//! Run with `cargo run --release -p msrtc-rans-bench`. Workloads are
//! deterministic (seeded LCG); results are medians of `--iterations` runs.

#![forbid(unsafe_code)]
#![allow(missing_docs)]

use std::time::{Duration, Instant};

use msrtc_rans::entropy::{EntropyDecoder, EntropyEncoder};
use msrtc_rans::stream::{RansDecoderStream, RansEncoderStream};
use msrtc_rans_core::sink::VecSink;
use msrtc_rans_core::source::SliceSource;
use msrtc_rans_core::{
    Freq, Rans64Decoder, Rans64EncSymbol, Rans64Encoder, RansByteDecoder, RansByteEncSymbol,
    RansByteEncoder,
};

/// Deterministic xorshift64* generator.
struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Lcg(seed.max(1))
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    fn below(&mut self, bound: u64) -> u64 {
        if bound == 0 { 0 } else { self.next() % bound }
    }
}

fn gen_symbols(rng: &mut Lcg, scale: u32, count: usize) -> Vec<(u32, u32)> {
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let start = rng.below(scale as u64) as u32;
        let mut max_freq = scale - start;
        if max_freq == scale {
            max_freq -= 1;
        }
        let freq = match rng.below(4) {
            0 => 1,
            1 => max_freq,
            2 => (max_freq / 2).max(1),
            _ => 1 + rng.below(max_freq as u64) as u32,
        };
        out.push((start, freq.clamp(1, max_freq)));
    }
    out
}

fn make_pmf(
    rng: &mut Lcg,
    scale: u32,
    center: i32,
    dist_count: usize,
) -> (Vec<i32>, Vec<i32>, Vec<i32>) {
    let mut lengths = Vec::new();
    let mut offsets = Vec::new();
    let mut table = Vec::new();
    for _ in 0..dist_count {
        let mut pmf: Vec<f64> = (0..=2 * center as usize)
            .map(|i| 1.0 / (1.0 + ((i as i64 - center as i64) as f64) * 0.25))
            .collect();
        pmf.push(pmf[0]);
        let sum: f64 = pmf.iter().sum();
        let mut base: Vec<i64> = pmf
            .iter()
            .map(|p| ((p / sum) * scale as f64).floor() as i64)
            .collect();
        for b in base.iter_mut() {
            *b = (*b).max(1);
        }
        let mut total: i64 = base.iter().sum();
        let target = scale as i64;
        let mut idx = 0usize;
        let len = base.len();
        while total < target {
            base[idx % len] += 1;
            total += 1;
            idx += 1;
        }
        while total > target {
            if let Some((i, _)) = base.iter().enumerate().find(|(_, b)| **b > 1) {
                base[i] -= 1;
                total -= 1;
            } else {
                break;
            }
        }
        lengths.push(base.len() as i32);
        offsets.push(-center);
        table.extend(base.iter().map(|b| *b as i32));
    }
    (lengths, offsets, table)
}

// ---------------------------------------------------------------------------
// Benchmark runner
// ---------------------------------------------------------------------------

struct Bench {
    name: &'static str,
    iterations: usize,
}

impl Bench {
    fn run(&self, mut f: impl FnMut() -> (u64, u64)) -> (Duration, u64, u64) {
        // Warmup
        for _ in 0..2 {
            let _ = f();
        }
        let mut best = Duration::MAX;
        let mut items = 0u64;
        let mut bytes = 0u64;
        for _ in 0..self.iterations {
            let start = Instant::now();
            let (i, b) = f();
            let elapsed = start.elapsed();
            if elapsed < best {
                best = elapsed;
                items = i;
                bytes = b;
            }
        }
        (best, items, bytes)
    }
}

fn report(name: &str, elapsed: Duration, items: u64, bytes: u64) {
    let secs = elapsed.as_secs_f64();
    let items_per_s = items as f64 / secs;
    let mb_per_s = bytes as f64 / secs / 1e6;
    println!(
        "{:<38} {:>10.2} M items/s {:>10.2} MB/s  ({} items, {} bytes in {:.3}s)",
        name,
        items_per_s / 1e6,
        mb_per_s,
        items,
        bytes,
        secs
    );
}

fn main() {
    let iterations = std::env::args()
        .nth(1)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(25);

    println!(
        "msrtc-rans-rs benchmark ({} iterations, release)",
        iterations
    );
    println!("{}", "-".repeat(100));

    // ---- raw RansByte encode / decode ----
    let mut rng = Lcg::new(0x7E17);
    let symbols = gen_symbols(&mut rng, 1 << 16, 100_000);
    let bytes_per = symbols.len() as u64;

    let b = Bench {
        name: "raw_byte_encode",
        iterations,
    };
    let (el, i, by) = b.run(|| {
        let sink = VecSink::<u8>::new(4096);
        let mut enc = RansByteEncoder::new(sink);
        for &(s, f) in &symbols {
            enc.put_raw(s, f, 16);
        }
        enc.flush();
        let out = enc.into_sink().encoded().to_vec();
        (symbols.len() as u64, out.len() as u64)
    });
    report("raw RansByte encode", el, i, by);

    // Pre-encoded stream for decode benches
    let sink = VecSink::<u8>::new(4096);
    let mut enc = RansByteEncoder::new(sink);
    for &(s, f) in &symbols {
        enc.put_raw(s, f, 16);
    }
    enc.flush();
    let byte_stream = enc.into_sink().encoded().to_vec();

    let b = Bench {
        name: "raw_byte_decode",
        iterations,
    };
    let (el, i, by) = b.run(|| {
        let src = SliceSource::new(&byte_stream[..]);
        let mut dec = RansByteDecoder::new(src);
        assert!(dec.init());
        let mut n = 0u64;
        for &(s, f) in symbols.iter().rev() {
            let _ = dec.get(16);
            dec.advance(s, f, 16);
            n += 1;
        }
        (n, byte_stream.len() as u64)
    });
    report("raw RansByte decode", el, i, by);

    // ---- raw Rans64 encode / decode ----
    let mut rng = Lcg::new(0x64E7);
    let symbols64 = gen_symbols(&mut rng, 1 << 24, 100_000);

    let b = Bench {
        name: "raw_64_encode",
        iterations,
    };
    let (el, i, by) = b.run(|| {
        let sink = VecSink::<u32>::new(4096);
        let mut enc = Rans64Encoder::new(sink);
        for &(s, f) in &symbols64 {
            enc.put_raw(s, f, 24);
        }
        enc.flush();
        let out = enc.into_sink().encoded().to_vec();
        (symbols64.len() as u64, out.len() as u64)
    });
    report("raw Rans64 encode", el, i, by);

    let sink = VecSink::<u32>::new(4096);
    let mut enc = Rans64Encoder::new(sink);
    for &(s, f) in &symbols64 {
        enc.put_raw(s, f, 24);
    }
    enc.flush();
    let stream64 = enc.into_sink().encoded().to_vec();

    let b = Bench {
        name: "raw_64_decode",
        iterations,
    };
    let (el, i, by) = b.run(|| {
        let src = SliceSource::new(&stream64[..]);
        let mut dec = Rans64Decoder::new(src);
        assert!(dec.init());
        let mut n = 0u64;
        for &(s, f) in symbols64.iter().rev() {
            let _ = dec.get(24);
            dec.advance(s, f, 24);
            n += 1;
        }
        (n, stream64.len() as u64)
    });
    report("raw Rans64 decode", el, i, by);

    // ---- prepared symbol encode ----
    let mut rng = Lcg::new(0x9E7);
    let prepared: Vec<RansByteEncSymbol> = symbols
        .iter()
        .map(|&(s, f)| RansByteEncSymbol::new(s, f, 16))
        .collect();
    let b = Bench {
        name: "prepared_byte_encode",
        iterations,
    };
    let (el, i, by) = b.run(|| {
        let sink = VecSink::<u8>::new(4096);
        let mut enc = RansByteEncoder::new(sink);
        for sym in &prepared {
            enc.put(sym);
        }
        enc.flush();
        let out = enc.into_sink().encoded().to_vec();
        (symbols.len() as u64, out.len() as u64)
    });
    report("prepared RansByte encode", el, i, by);

    let prepared64: Vec<Rans64EncSymbol> = symbols64
        .iter()
        .map(|&(s, f)| Rans64EncSymbol::new(s, f, 24))
        .collect();
    let b = Bench {
        name: "prepared_64_encode",
        iterations,
    };
    let (el, i, by) = b.run(|| {
        let sink = VecSink::<u32>::new(4096);
        let mut enc = Rans64Encoder::new(sink);
        for sym in &prepared64 {
            enc.put(sym);
        }
        enc.flush();
        let out = enc.into_sink().encoded().to_vec();
        (symbols64.len() as u64, out.len() as u64)
    });
    report("prepared Rans64 encode", el, i, by);

    // ---- entropy encode / decode ----
    let mut rng = Lcg::new(0xE7E7);
    let (lengths, offsets, table) = make_pmf(&mut rng, 1 << 16, 8, 4);
    let n_values = 100_000usize;
    let mut values = Vec::with_capacity(n_values);
    let mut indices = Vec::with_capacity(n_values);
    for i in 0..n_values {
        indices.push((i % 4) as i32);
        values.push(match i % 20 {
            0 => 300 + (i as i32) % 100,    // positive outlier (bypass)
            1 => -(300 + (i as i32) % 100), // negative outlier (bypass)
            _ => ((i as i32) % 17) - 8,
        });
    }

    // Byte entropy
    let mut enc_byte: EntropyEncoder<msrtc_rans_core::variant::RansByte> = EntropyEncoder::new();
    enc_byte
        .initialize(&lengths, &offsets, &table, 16, 4)
        .unwrap();
    let mut buf = Vec::new();
    enc_byte.encode(&indices, &values, &mut buf).unwrap();

    let b = Bench {
        name: "entropy_byte_encode",
        iterations,
    };
    let (el, i, by) = b.run(|| {
        let mut buf = Vec::new();
        enc_byte.encode(&indices, &values, &mut buf).unwrap();
        (values.len() as u64, buf.len() as u64)
    });
    report("entropy RansByte encode", el, i, by);

    let mut dec_byte: EntropyDecoder<msrtc_rans_core::variant::RansByte> = EntropyDecoder::new();
    dec_byte
        .initialize(&lengths, &offsets, &table, 16, 4)
        .unwrap();
    let b = Bench {
        name: "entropy_byte_decode",
        iterations,
    };
    let (el, i, by) = b.run(|| {
        let mut out = vec![0i32; values.len()];
        dec_byte.decode(&mut out, &indices, &buf).unwrap();
        (values.len() as u64, buf.len() as u64)
    });
    report("entropy RansByte decode", el, i, by);

    // Rans64 entropy
    let mut enc_64: EntropyEncoder<msrtc_rans_core::variant::Rans64> = EntropyEncoder::new();
    enc_64
        .initialize(&lengths, &offsets, &table, 16, 4)
        .unwrap();
    let mut buf64 = Vec::new();
    enc_64.encode(&indices, &values, &mut buf64).unwrap();

    let b = Bench {
        name: "entropy_64_encode",
        iterations,
    };
    let (el, i, by) = b.run(|| {
        let mut buf = Vec::new();
        enc_64.encode(&indices, &values, &mut buf).unwrap();
        (values.len() as u64, buf.len() as u64)
    });
    report("entropy Rans64 encode", el, i, by);

    let mut dec_64: EntropyDecoder<msrtc_rans_core::variant::Rans64> = EntropyDecoder::new();
    dec_64
        .initialize(&lengths, &offsets, &table, 16, 4)
        .unwrap();
    let b = Bench {
        name: "entropy_64_decode",
        iterations,
    };
    let (el, i, by) = b.run(|| {
        let mut out = vec![0i32; values.len()];
        dec_64.decode(&mut out, &indices, &buf64).unwrap();
        (values.len() as u64, buf64.len() as u64)
    });
    report("entropy Rans64 decode", el, i, by);

    // ---- persistent stream multipart ----
    let mut rng = Lcg::new(0x57E7);
    let mut batches = Vec::new();
    for b_ in 0..8 {
        let (l, o, t) = make_pmf(&mut rng, 1 << 16, 8, 2);
        let n = 12_500usize;
        let mut v = Vec::with_capacity(n);
        let mut ix = Vec::with_capacity(n);
        for i in 0..n {
            ix.push((i % 2) as i32);
            v.push(((i as i32) % 17) - 8 + if i % 50 == 0 { 100 } else { 0 });
        }
        batches.push((l, o, t, ix, v));
    }
    let total_stream_values: u64 = batches.iter().map(|(_, _, _, ix, _)| ix.len() as u64).sum();

    let b = Bench {
        name: "stream_multipart_encode",
        iterations,
    };
    let (el, i, by) = b.run(|| {
        let mut stream = RansEncoderStream::<msrtc_rans_core::variant::RansByte>::new();
        for (l, o, t, ix, v) in &batches {
            let mut e: EntropyEncoder<msrtc_rans_core::variant::RansByte> = EntropyEncoder::new();
            e.initialize(l, o, t, 16, 4).unwrap();
            stream.push(&e, ix, v).unwrap();
        }
        let data = stream.flush().unwrap();
        (total_stream_values, data.len() as u64)
    });
    report("stream RansByte multipart encode", el, i, by);

    let mut stream = RansEncoderStream::<msrtc_rans_core::variant::RansByte>::new();
    for (l, o, t, ix, v) in &batches {
        let mut e: EntropyEncoder<msrtc_rans_core::variant::RansByte> = EntropyEncoder::new();
        e.initialize(l, o, t, 16, 4).unwrap();
        stream.push(&e, ix, v).unwrap();
    }
    let stream_data = stream.flush().unwrap();

    let b = Bench {
        name: "stream_multipart_decode",
        iterations,
    };
    let (el, i, by) = b.run(|| {
        let mut ds = RansDecoderStream::<msrtc_rans_core::variant::RansByte>::open_on(&stream_data);
        for (l, o, t, ix, expected) in batches.iter().rev() {
            let mut d: EntropyDecoder<msrtc_rans_core::variant::RansByte> = EntropyDecoder::new();
            d.initialize(l, o, t, 16, 4).unwrap();
            let mut out = vec![0i32; expected.len()];
            ds.decode(&d, &mut out, ix).unwrap();
        }
        ds.decode_eof().unwrap();
        (total_stream_values, stream_data.len() as u64)
    });
    report("stream RansByte multipart decode", el, i, by);
}
