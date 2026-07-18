//! Hash every regular file in a directory concurrently, preserving input
//! order in the results, and report aggregate throughput.
//!
//! Run: `cargo run --release --example hash_batch -- <dir>`
//! Without an argument, 8 temporary 8 MiB files are generated and hashed.
//!
//! Covers: [`CoreHasher::hash_files`] (order-preserving, per-file
//! `Result`s — a missing file is demonstrated deliberately).

use core_blake3::CoreHasher;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

/// Removes generated temp files on scope exit.
struct TempDirFiles(Vec<PathBuf>);

impl Drop for TempDirFiles {
    fn drop(&mut self) {
        for p in &self.0 {
            let _ = std::fs::remove_file(p);
        }
    }
}

fn generate_temp_files() -> std::io::Result<TempDirFiles> {
    const COUNT: usize = 8;
    const SIZE: usize = 8 * 1024 * 1024;
    let block: Vec<u8> = (0..1 << 16).map(|i| (i % 251) as u8).collect();
    let mut paths = Vec::new();
    for i in 0..COUNT {
        let path = std::env::temp_dir().join(format!("core-blake3-batch-{}-{i:02}.bin", std::process::id()));
        let mut f = std::fs::File::create(&path)?;
        for _ in 0..SIZE / block.len() {
            f.write_all(&block)?;
        }
        // Make each file distinct so the digests differ.
        f.write_all(&[i as u8])?;
        paths.push(path);
    }
    println!("(no argument given: generated {COUNT} temp files of {} MiB each)", SIZE >> 20);
    Ok(TempDirFiles(paths))
}

fn main() -> std::io::Result<()> {
    let (mut paths, _cleanup) = match std::env::args().nth(1) {
        Some(dir) => {
            let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)?
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_file())
                .collect();
            paths.sort();
            (paths, None)
        }
        None => {
            let tmp = generate_temp_files()?;
            (tmp.0.clone(), Some(tmp))
        }
    };

    if paths.is_empty() {
        eprintln!("no files to hash");
        std::process::exit(1);
    }

    // Deliberately include a nonexistent path to show per-file error
    // handling: one bad entry must not poison the rest of the batch.
    paths.push(PathBuf::from("/nonexistent/core-blake3-demo"));

    let hasher = CoreHasher::new();
    println!("Hashing {} files with {} threads (one per physical core)\n", paths.len(), hasher.threads());

    let start = Instant::now();
    let results = hasher.hash_files(&paths);
    let elapsed = start.elapsed();

    let mut ok_bytes = 0u64;
    for (path, result) in paths.iter().zip(&results) {
        match result {
            Ok(hash) => {
                ok_bytes += std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                println!("{}  {}", hash.to_hex(), path.display());
            }
            Err(e) => println!("ERROR: {} ({e})", path.display()),
        }
    }

    println!(
        "\n{:.1} MiB in {:.2} ms -> {:.0} MiB/s aggregate (includes reading the files)",
        ok_bytes as f64 / (1024.0 * 1024.0),
        elapsed.as_secs_f64() * 1e3,
        ok_bytes as f64 / (1024.0 * 1024.0) / elapsed.as_secs_f64()
    );
    Ok(())
}
