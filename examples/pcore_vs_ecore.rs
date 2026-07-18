//! Compare BLAKE3 throughput when the hasher is pinned to performance
//! cores vs efficiency cores. Hybrid machines only — on a homogeneous CPU
//! it explains why there's nothing to compare and exits cleanly.
//!
//! Run: `cargo run --release --example pcore_vs_ecore`
//!
//! Covers: [`PcoreHasher::with_cpus`], [`PcoreHasher::hash_bytes`],
//! [`topology`] gating.
//!
//! This is a demo, not a benchmark: single buffer, best-of-5 timing, no
//! shuffling or cache control. For rigorous numbers, see the methodology
//! in the README's "Why" section.

use pcore_blake3::{efficiency_cpus, performance_cpus, topology, PcoreHasher, Topology};
use std::time::{Duration, Instant};

const SIZE: usize = 256 * 1024 * 1024;
const REPS: usize = 5;

fn best_of(reps: usize, mut f: impl FnMut()) -> Duration {
    let mut best = Duration::MAX;
    for _ in 0..reps {
        let start = Instant::now();
        f();
        best = best.min(start.elapsed());
    }
    best
}

fn mib_s(bytes: usize, d: Duration) -> f64 {
    bytes as f64 / (1024.0 * 1024.0) / d.as_secs_f64()
}

fn main() {
    if topology() != Topology::Hybrid {
        println!("This machine reports a homogeneous CPU topology: every core is the");
        println!("same kind, so a P-core vs E-core comparison is not possible here.");
        return;
    }
    let p_cpus = performance_cpus();
    let e_cpus = efficiency_cpus();

    println!("Buffer: {} MiB, best of {REPS} runs each\n", SIZE >> 20);
    let data: Vec<u8> = (0..SIZE).map(|i| (i % 251) as u8).collect();

    let p_hasher = PcoreHasher::with_cpus(&p_cpus);
    let e_hasher = PcoreHasher::with_cpus(&e_cpus);
    let (p_tpf, p_cf) = p_hasher.split();
    let (e_tpf, e_cf) = e_hasher.split();

    // Warm-up (page in the buffer, spin up the pools) before timing.
    let p_digest = p_hasher.hash_bytes(&data);
    let e_digest = e_hasher.hash_bytes(&data);
    assert_eq!(p_digest, e_digest, "digest must not depend on which cores computed it");

    let t_single = best_of(REPS, || {
        blake3::hash(&data);
    });
    let t_p = best_of(REPS, || {
        p_hasher.hash_bytes(&data);
    });
    let t_e = best_of(REPS, || {
        e_hasher.hash_bytes(&data);
    });

    println!("{:<34} {:>12} {:>12}", "configuration", "time (ms)", "MiB/s");
    println!(
        "{:<34} {:>12.1} {:>12.0}",
        "single thread (reference)",
        t_single.as_secs_f64() * 1e3,
        mib_s(SIZE, t_single)
    );
    println!(
        "{:<34} {:>12.1} {:>12.0}",
        format!("{} P-core threads ({p_tpf}x{p_cf})", p_cpus.len()),
        t_p.as_secs_f64() * 1e3,
        mib_s(SIZE, t_p)
    );
    println!(
        "{:<34} {:>12.1} {:>12.0}",
        format!("{} E-core threads ({e_tpf}x{e_cf})", e_cpus.len()),
        t_e.as_secs_f64() * 1e3,
        mib_s(SIZE, t_e)
    );

    println!(
        "\nP-core pool is {:.2}x the E-core pool on this machine (digests identical).",
        t_e.as_secs_f64() / t_p.as_secs_f64()
    );
}
