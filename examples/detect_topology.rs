//! Inspect this machine's CPU topology as pcore-blake3 sees it, and show
//! what the threads/2 heuristic would pick across a range of machine sizes.
//!
//! Run: `cargo run --release --example detect_topology`
//!
//! Covers: [`topology`], [`performance_cpus`], [`efficiency_cpus`],
//! [`optimal_split`], [`PcoreHasher::split`].

use pcore_blake3::{efficiency_cpus, optimal_split, performance_cpus, topology, PcoreHasher, Topology};

fn main() {
    let topo = topology();
    let p = performance_cpus();
    let e = efficiency_cpus();

    println!("Topology          : {topo:?}");
    println!("Performance cores : {p:?}");
    println!("Efficiency cores  : {e:?}");

    let hasher = PcoreHasher::new();
    let (tpf, cf) = hasher.split();
    println!("Chosen split      : {tpf} threads/file x {cf} concurrent files");
    if topo == Topology::Hybrid {
        println!("                    (built on the {} P-core threads only)", p.len());
    }

    println!("\nthreads/2 heuristic across machine sizes:");
    println!("{:>8} {:>14} {:>18}", "threads", "threads/file", "concurrent files");
    for threads in [2usize, 4, 6, 8, 10, 12, 16, 20, 24, 32] {
        let (tpf, cf) = optimal_split(threads);
        println!("{threads:>8} {tpf:>14} {cf:>18}");
    }
}
