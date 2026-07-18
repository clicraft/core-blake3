# pcore-blake3

BLAKE3 hashing that auto-detects performance ("P") cores on hybrid CPUs
(Intel Alder Lake+ and beyond, and AMD hybrid parts on Windows) and picks
a thread split between BLAKE3's internal tree parallelism and
concurrent-file parallelism, instead of blending P-cores and E-cores into
one undifferentiated "use all cores" pool.

## Why

On a hybrid CPU, "use all logical CPUs" mixes two different core speeds
into one throughput number, and it isn't obvious how to split N available
threads between "one file's internal BLAKE3 tree" and "how many files
hash concurrently" — 1 thread/file and N threads/file are usually both
wrong. Benchmarked on a real 13th-gen Intel i9 (6 P-cores / 12 threads, 8
E-cores) across P-core counts from 2 to 6:

| P-cores | threads | best split |
|---|---|---|
| 2 | 4 | 4 threads/file x 1 file (no split) |
| 3 | 6 | 3 threads/file x 2 files |
| 4 | 8 | ~4 threads/file x 2 files |
| 5 | 10 | ~5 threads/file x 2 files |
| 6 | 12 | ~3-4 threads/file x 3-4 files |

The pattern: **`threads / 2`, snapped to the nearest divisor of the
thread count, from 4 threads up; no file-splitting below that.** BLAKE3
hashing with this split beat hardware-accelerated (SHA-NI) SHA-256 by
roughly 1.5-2x in every fair, same-thread-count comparison run during
this benchmarking.

`optimal_split()` in this crate implements exactly that heuristic.

## Usage

```rust
use pcore_blake3::PcoreHasher;

let hasher = PcoreHasher::new(); // auto-detects P-cores, picks the split
let hash = hasher.hash_file("document.pdf")?;
println!("{}", hash.to_hex());

// Or a batch, spread across the hasher's pinned pools:
let hashes = hasher.hash_files(&["a.pdf".into(), "b.pdf".into()]);
```

CLI:

```sh
pcore-blake3 file1.pdf file2.pdf
pcore-blake3 --info   # print detected topology and thread split
```

## Platform support

- **Linux**: detection via the kernel's hybrid-core sysfs markers
  (`/sys/devices/cpu_core/cpus`, `/sys/devices/cpu_atom/cpus`, present
  since Linux 5.16 on Intel hybrid parts). **Verified on real hardware.**
- **Windows**: detection via `GetSystemCpuSetInformation` and each
  logical processor's `EfficiencyClass` (a Windows scheduler
  abstraction — vendor-agnostic, so it covers AMD hybrid parts too, not
  just Intel). Typechecked and clippy-clean against
  `x86_64-pc-windows-gnu`, and CI exercises it on `windows-latest`
  runners (homogeneous CPUs), but **not yet run on real hybrid Windows
  hardware** — see [PORT_VALIDATION.md](PORT_VALIDATION.md) for the full
  validation status and remaining checklist.
- Any other platform: falls back to treating every available CPU as a
  performance core (no pinning, no P/E distinction) — this crate still
  works, it just can't do anything smarter than "use all cores."

## License

Licensed under either of

- MIT license ([LICENSE-MIT](LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option.
