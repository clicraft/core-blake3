# pcore-blake3

[![CI](https://github.com/clicraft/pcore-blake3/actions/workflows/ci.yml/badge.svg)](https://github.com/clicraft/pcore-blake3/actions/workflows/ci.yml)

Fast BLAKE3 file hashing that pins work to your CPU's cores. On hybrid CPUs
(Intel P/E-core, AMD) it detects performance vs efficiency cores and runs
**one thread per physical core** — which our analysis found is the
throughput sweet spot for BLAKE3: it saturates each core's SIMD units, so a
second hardware thread per core barely helps.

Library + CLI, pure Rust. The hashing itself is the official
[`blake3`](https://crates.io/crates/blake3) crate; this crate adds the
core detection, pinning, and scheduling around it.

## Install

```toml
[dependencies]
pcore-blake3 = { git = "https://github.com/clicraft/pcore-blake3", tag = "v0.4.0" }
```

CLI: `cargo install --git https://github.com/clicraft/pcore-blake3`, or grab
a prebuilt Linux/Windows binary from
[Releases](https://github.com/clicraft/pcore-blake3/releases).

## Library usage

```rust
use pcore_blake3::PcoreHasher;

let hasher = PcoreHasher::new();          // pins to the performance cores
let hash = hasher.hash_file("doc.pdf")?;  // one file
let hash = hasher.hash_bytes(b"data");    // in-memory buffer

// A batch: results come back in input order, one io::Result per file.
let hashes = hasher.hash_files(&["a.pdf".into(), "b.pdf".into()]);
```

Digests are identical to `blake3::hash` — the parallel scheduling never
changes the result.

### Modes

| constructor | cores used | when |
|---|---|---|
| `PcoreHasher::new()` | performance cores | **default** — great for single files and I/O-bound batches |
| `PcoreHasher::new_physical()` | one thread per physical P-core | smaller thread footprint |
| `PcoreHasher::new_all_physical()` | one thread per physical core, P **and** E | maximum throughput on large batches (uses the E-cores) |

## CLI

```console
$ pcore-blake3 --info
Topology: hybrid
Performance cores: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11] (12 threads, 6 physical)
Efficiency cores: [12, 13, 14, 15, 16, 17, 18, 19] (8 threads)
All physical cores (P+E): 14 (for --all-physical)
Thread split: 6 threads/file x 2 concurrent files

$ pcore-blake3 doc1.pdf doc2.pdf          # b3sum-compatible output
$ pcore-blake3 --all-physical *.pdf       # max throughput, uses E-cores
```

## Examples

Self-contained (generate their own data when run without arguments):

| example | shows |
|---|---|
| `detect_topology` | detected cores and the chosen thread layout |
| `hash_file` | single file + throughput, checked against `blake3::hash` |
| `hash_batch` | order-preserving directory batch, per-file error isolation |
| `pcore_vs_ecore` | P-core vs E-core throughput on hybrid CPUs |

Run with `cargo run --release --example <name>`.

## API overview

| item | purpose |
|---|---|
| `topology() -> Topology` | `Hybrid` or `Homogeneous` |
| `performance_cpus()` / `efficiency_cpus()` | logical CPU ids of P- / E-cores |
| `performance_physical_cpus()` | one logical CPU per physical P-core |
| `all_physical_cpus()` | one logical CPU per physical core, P and E |
| `physical_core_leaders(&[usize])` | collapse SMT siblings in any CPU set |
| `pin_current_thread_to_cpu(usize)` | pin the calling thread to one CPU |
| `optimal_split(threads)` | the thread-split heuristic |
| `PcoreHasher::new` / `new_physical` / `new_all_physical` / `with_cpus` | build a hasher |
| `PcoreHasher::hash_bytes` / `hash_file` / `hash_files` | hash |

## Platform support

- **Linux**: verified on real hybrid hardware (Intel i9-13900HK). Detection
  via the kernel's hybrid-core sysfs markers; pinning via
  `sched_setaffinity`.
- **Windows**: detection via `GetSystemCpuSetInformation` (vendor-agnostic —
  Intel and AMD); built and tested in CI on `windows-latest`, but the
  hybrid-topology path hasn't yet run on real hybrid Windows hardware — see
  [PORT_VALIDATION.md](PORT_VALIDATION.md).
- Other platforms: works, treating every CPU as a performance core.

## Validation

The core-detection code is a Rust port of a C reference implementation,
validated against it by differential testing — see
[PORT_VALIDATION.md](PORT_VALIDATION.md). Changes are in
[CHANGELOG.md](CHANGELOG.md).

## License

MIT ([LICENSE-MIT](LICENSE-MIT)) or Apache-2.0 ([LICENSE-APACHE](LICENSE-APACHE)), at your option.
