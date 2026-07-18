use core_blake3::{CoreHasher, Topology};
use std::path::PathBuf;
use std::process::ExitCode;

fn print_usage(prog: &str) {
    eprintln!("Usage: {prog} [--info] [--all-threads] <file>...");
    eprintln!("  Hashes each file with BLAKE3, pinning one thread per physical core.");
    eprintln!("  Prints \"<hex-digest>  <path>\" per file (b3sum-compatible).");
    eprintln!("  --info         print detected CPU topology and thread count, then exit");
    eprintln!("  --all-threads  use every logical CPU (all SMT threads) instead");
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let prog = args.first().map(String::as_str).unwrap_or("core-blake3");

    let mut all_threads = false;
    let mut info = false;
    let mut paths: Vec<PathBuf> = Vec::new();
    for arg in &args[1..] {
        match arg.as_str() {
            "--all-threads" => all_threads = true,
            "--info" => info = true,
            _ => paths.push(PathBuf::from(arg)),
        }
    }

    let hasher = if all_threads { CoreHasher::all_threads() } else { CoreHasher::new() };

    if info {
        print_info(&hasher);
        return ExitCode::SUCCESS;
    }

    if paths.is_empty() {
        print_usage(prog);
        return ExitCode::FAILURE;
    }

    let results = hasher.hash_files(&paths);

    let mut ok = true;
    for (path, result) in paths.iter().zip(results) {
        match result {
            Ok(hash) => println!("{}  {}", hash.to_hex(), path.display()),
            Err(e) => {
                eprintln!("{}: {e}", path.display());
                ok = false;
            }
        }
    }

    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn print_info(hasher: &CoreHasher) {
    let topology = core_blake3::topology();
    let p_cpus = core_blake3::performance_cpus();
    let e_cpus = core_blake3::efficiency_cpus();
    let phys = core_blake3::all_physical_cpus();
    let logical = core_blake3::all_logical_cpus();

    println!("Topology: {}", if topology == Topology::Hybrid { "hybrid" } else { "homogeneous" });
    println!("Performance cores: {p_cpus:?}");
    println!("Efficiency cores: {e_cpus:?}");
    println!("Physical cores (P+E): {}   Logical CPUs: {}", phys.len(), logical.len());
    println!("This run: {} threads ({})", hasher.threads(), if hasher.threads() == logical.len() { "all logical" } else { "one per physical core" });
}
