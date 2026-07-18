//! Inspect this machine's CPU topology as core-blake3 sees it, and the
//! thread count each of the two modes would use.
//!
//! Run: `cargo run --release --example detect_topology`
//!
//! Covers: [`topology`], [`performance_cpus`], [`efficiency_cpus`],
//! [`all_physical_cpus`], [`all_logical_cpus`], [`CoreHasher::threads`].

use core_blake3::{
    all_logical_cpus, all_physical_cpus, efficiency_cpus, performance_cpus, topology, CoreHasher,
};

fn main() {
    println!("Topology          : {:?}", topology());
    println!("Performance cores : {:?}", performance_cpus());
    println!("Efficiency cores  : {:?}", efficiency_cpus());
    println!("Physical cores    : {:?} ({} cores)", all_physical_cpus(), all_physical_cpus().len());
    println!("Logical CPUs      : {:?} ({} threads)", all_logical_cpus(), all_logical_cpus().len());

    println!("\nModes:");
    println!("  new()          -> {:>2} threads (one per physical core, the efficient default)", CoreHasher::new().threads());
    println!("  all_threads()  -> {:>2} threads (every logical CPU, the conventional baseline)", CoreHasher::all_threads().threads());
}
