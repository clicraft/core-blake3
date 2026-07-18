use pcore_blake3::{PcoreHasher, Topology};
use std::path::PathBuf;
use std::process::ExitCode;

fn print_usage(prog: &str) {
    eprintln!("Usage: {prog} [--info] <file>...");
    eprintln!("  Hashes each file with BLAKE3, using this machine's performance cores");
    eprintln!("  and an optimal thread split. Prints \"<hex-digest>  <path>\" per file.");
    eprintln!("  --info   print detected CPU topology and thread split, then exit");
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let prog = args.first().map(String::as_str).unwrap_or("pcore-blake3");

    if args.get(1).map(String::as_str) == Some("--info") {
        print_info();
        return ExitCode::SUCCESS;
    }

    let paths: Vec<PathBuf> = args[1..].iter().map(PathBuf::from).collect();
    if paths.is_empty() {
        print_usage(prog);
        return ExitCode::FAILURE;
    }

    let hasher = PcoreHasher::new();
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

fn print_info() {
    let topology = pcore_blake3::topology();
    let p_cpus = pcore_blake3::performance_cpus();
    let e_cpus = pcore_blake3::efficiency_cpus();
    let hasher = PcoreHasher::new();
    let (tpf, cf) = hasher.split();

    println!("Topology: {}", if topology == Topology::Hybrid { "hybrid" } else { "homogeneous" });
    println!("Performance cores: {p_cpus:?}");
    println!("Efficiency cores: {e_cpus:?}");
    println!("Thread split: {tpf} threads/file x {cf} concurrent files");
}
