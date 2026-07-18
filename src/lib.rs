//! Core-aware BLAKE3 file hashing.
//!
//! Detects the machine's CPU topology and pins hashing threads to cores.
//! The hashing itself is the official [`blake3`] crate; this crate places
//! the work. Two modes, both spanning every core (P and E on a hybrid CPU):
//!
//! * [`CoreHasher::new`] — **one thread per physical core** (SMT siblings
//!   collapsed). Our analysis found this is BLAKE3's sweet spot: it
//!   saturates each core's SIMD units, so a second hardware thread per core
//!   barely helps. Fewer threads, same-or-better throughput.
//! * [`CoreHasher::all_threads`] — **one thread per logical CPU** (every
//!   SMT thread). The conventional "use everything" baseline.
//!
//! ```no_run
//! use core_blake3::CoreHasher;
//!
//! let hasher = CoreHasher::new();
//! let hash = hasher.hash_file("document.pdf").unwrap();
//! println!("{}", hash.to_hex());
//! ```

mod affinity;

pub use affinity::{
    all_logical_cpus, all_physical_cpus, efficiency_cpus, performance_cpus,
    performance_physical_cpus, physical_core_leaders, pin_current_thread_to_cpu, topology, Topology,
};

use rayon::prelude::*;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Builds a rayon thread pool whose worker threads are each pinned, in
/// round-robin order, to one CPU from `cpus`.
fn build_pinned_pool(cpus: Vec<usize>) -> rayon::ThreadPool {
    let num_threads = cpus.len().max(1);
    let counter = Arc::new(AtomicUsize::new(0));
    let cpus = Arc::new(cpus);
    rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .spawn_handler(move |thread| {
            let counter = Arc::clone(&counter);
            let cpus = Arc::clone(&cpus);
            std::thread::Builder::new().spawn(move || {
                if !cpus.is_empty() {
                    let idx = counter.fetch_add(1, Ordering::SeqCst);
                    let _ = pin_current_thread_to_cpu(cpus[idx % cpus.len()]);
                }
                thread.run();
            })?;
            Ok(())
        })
        .build()
        .expect("build pinned rayon thread pool")
}

/// A BLAKE3 hasher backed by one rayon pool whose threads are pinned to a
/// chosen set of CPUs. A single file is hashed with BLAKE3's tree
/// parallelism across the whole pool; a batch is hashed one-file-per-thread
/// (rayon work-steals, so fast cores take more files and slow ones fewer —
/// no straggler ever holds up a single file).
pub struct CoreHasher {
    pool: rayon::ThreadPool,
    threads: usize,
}

impl CoreHasher {
    /// **One thread per physical core**, across every core on the machine
    /// (P and E), SMT siblings collapsed. The efficient, recommended
    /// default. Falls back to all logical CPUs if topology can't be read.
    pub fn new() -> Self {
        let cpus = all_physical_cpus();
        if cpus.is_empty() {
            return Self::all_threads();
        }
        Self::with_cpus(&cpus)
    }

    /// **One thread per logical CPU** — every SMT thread on every core. The
    /// conventional "use all threads" baseline; usually a touch slower than
    /// [`Self::new`] and always a larger thread footprint.
    pub fn all_threads() -> Self {
        let cpus = all_logical_cpus();
        if cpus.is_empty() {
            return Self::with_cpus(&[0]);
        }
        Self::with_cpus(&cpus)
    }

    /// Pin to an explicit set of logical CPU IDs (one thread each). Escape
    /// hatch for testing or bespoke placement.
    pub fn with_cpus(cpus: &[usize]) -> Self {
        let threads = cpus.len().max(1);
        Self { pool: build_pinned_pool(cpus.to_vec()), threads }
    }

    /// Number of pinned worker threads.
    pub fn threads(&self) -> usize {
        self.threads
    }

    /// Hashes an in-memory buffer with BLAKE3 tree parallelism across the
    /// whole pool. The digest is identical to `blake3::hash(data)`.
    pub fn hash_bytes(&self, data: &[u8]) -> blake3::Hash {
        self.pool.install(|| {
            let mut hasher = blake3::Hasher::new();
            hasher.update_rayon(data);
            hasher.finalize()
        })
    }

    /// Hashes a single file (tree-parallel across the pool).
    pub fn hash_file(&self, path: impl AsRef<Path>) -> io::Result<blake3::Hash> {
        let data = std::fs::read(path)?;
        Ok(self.hash_bytes(&data))
    }

    /// Hashes many files, one file per thread (rayon distributes and
    /// work-steals across the pinned pool). Each file is read then hashed
    /// single-threaded, so slow cores simply handle fewer files. Results
    /// are returned in the same order as `paths`.
    pub fn hash_files(&self, paths: &[PathBuf]) -> Vec<io::Result<blake3::Hash>> {
        self.pool.install(|| {
            paths
                .par_iter()
                .map(|p| std::fs::read(p).map(|data| blake3::hash(&data)))
                .collect()
        })
    }
}

impl Default for CoreHasher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn new_uses_no_more_threads_than_all_threads() {
        assert!(CoreHasher::new().threads() <= CoreHasher::all_threads().threads());
    }

    #[test]
    fn both_modes_hash_correctly() {
        let data: Vec<u8> = (0..1 << 20).map(|i| (i % 251) as u8).collect();
        for h in [CoreHasher::new(), CoreHasher::all_threads()] {
            assert_eq!(h.hash_bytes(&data), blake3::hash(&data));
        }
    }

    #[test]
    fn hash_bytes_matches_reference_across_chunk_boundaries() {
        let hasher = CoreHasher::with_cpus(&[0, 1]);
        // Cover both sides of BLAKE3's 1024-byte chunk boundary and a
        // multi-chunk buffer where the parallel tree path actually engages.
        for len in [0usize, 1, 1023, 1024, 1025, 1 << 20] {
            let data: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
            assert_eq!(hasher.hash_bytes(&data), blake3::hash(&data), "len {len}");
        }
    }

    #[test]
    fn hash_file_matches_reference_blake3() {
        let tmp = tempfile_with_bytes(b"hello core-blake3");
        let hasher = CoreHasher::with_cpus(&[0, 1]);
        assert_eq!(hasher.hash_file(tmp.path()).unwrap(), blake3::hash(b"hello core-blake3"));
    }

    #[test]
    fn hash_files_matches_reference_and_preserves_order() {
        let contents: Vec<&[u8]> = vec![b"one", b"two", b"three", b"four", b"five"];
        let files: Vec<_> = contents.iter().map(|c| tempfile_with_bytes(c)).collect();
        let paths: Vec<PathBuf> = files.iter().map(|f| f.path().to_path_buf()).collect();

        // Use the real default hasher so the mixed-speed pool path is exercised.
        let results = CoreHasher::new().hash_files(&paths);
        assert_eq!(results.len(), contents.len());
        for (result, content) in results.into_iter().zip(contents) {
            assert_eq!(result.unwrap(), blake3::hash(content));
        }
    }

    struct TempFile {
        path: PathBuf,
    }
    impl TempFile {
        fn path(&self) -> &Path {
            &self.path
        }
    }
    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    fn tempfile_with_bytes(data: &[u8]) -> TempFile {
        let mut path = std::env::temp_dir();
        path.push(format!("core-blake3-test-{:p}-{}", data.as_ptr(), data.len()));
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(data).unwrap();
        TempFile { path }
    }
}
