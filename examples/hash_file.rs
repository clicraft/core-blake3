//! Hash a single file with the P-core-tuned hasher, report throughput, and
//! self-check the digest against the reference `blake3::hash`.
//!
//! Run: `cargo run --release --example hash_file -- <path>`
//! Without an argument, a 64 MiB temporary file is generated and hashed.
//!
//! Covers: [`CoreHasher::new`], [`CoreHasher::hash_file`],
//! [`CoreHasher::threads`].

use core_blake3::CoreHasher;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

/// Removes the generated temp file on scope exit.
struct TempFile(PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn generate_temp_file() -> std::io::Result<TempFile> {
    const SIZE: usize = 64 * 1024 * 1024;
    let path = std::env::temp_dir().join(format!("core-blake3-example-{}.bin", std::process::id()));
    let mut f = std::fs::File::create(&path)?;
    // Patterned, not all-zero, so the input resembles real data.
    let block: Vec<u8> = (0..1 << 16).map(|i| (i % 251) as u8).collect();
    for _ in 0..SIZE / block.len() {
        f.write_all(&block)?;
    }
    println!("(no argument given: generated {} MiB temp file {})", SIZE >> 20, path.display());
    Ok(TempFile(path))
}

fn main() -> std::io::Result<()> {
    let (path, _cleanup) = match std::env::args().nth(1) {
        Some(p) => (PathBuf::from(p), None),
        None => {
            let tmp = generate_temp_file()?;
            (tmp.0.clone(), Some(tmp))
        }
    };

    let hasher = CoreHasher::new();
    println!("Using {} threads (one per physical core)\n", hasher.threads());

    let size = std::fs::metadata(&path)?.len();
    let start = Instant::now();
    let hash = hasher.hash_file(&path)?;
    let elapsed = start.elapsed();

    println!("{}  {}", hash.to_hex(), path.display());
    println!(
        "{:.1} MiB in {:.2} ms -> {:.0} MiB/s (includes reading the file)",
        size as f64 / (1024.0 * 1024.0),
        elapsed.as_secs_f64() * 1e3,
        size as f64 / (1024.0 * 1024.0) / elapsed.as_secs_f64()
    );

    // Parallel tree hashing must be digest-identical to the reference.
    let reference = blake3::hash(&std::fs::read(&path)?);
    assert_eq!(hash, reference);
    println!("digest matches single-threaded blake3::hash reference: OK");
    Ok(())
}
