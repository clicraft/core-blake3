# C → Rust port validation report

Scope: `src/affinity.rs`, ported from the C reference implementation
`pcore-lib` (batchSigner repo: `pcore-lib/src/pcore.c`, `include/pcore.h`).
Goal: zero behavioral divergence between the two implementations, and zero
latent errors in the never-executed Windows path.

## Method

Five validation layers were applied, in order:

1. **Static side-by-side study** of both sources, producing a behavior
   matrix per function (topology / performance_cpus / efficiency_cpus /
   pin / cpulist parser).
2. **Runtime differential on real hardware** (Linux, i9-13900HK hybrid):
   C test binary vs Rust CLI vs raw sysfs ground truth.
3. **Parser differential harness**: the C `parse_cpu_list` exposed via a
   test harness and run against a 28-case corpus; the same corpus run
   through the Rust parser; outputs compared case by case.
4. **Windows compile validation**: `cargo check --target
   x86_64-pc-windows-gnu` (typechecks the entire Windows module against
   real `windows-sys` definitions — no VM needed).
5. **Regression encoding**: the corpus and behavioral invariants baked
   into the crate's test suite so future edits can't silently diverge.

## Findings

| # | Where | Severity | Finding | Resolution |
|---|-------|----------|---------|------------|
| F1 | Rust/Windows | compile error | `Process` argument of `GetSystemCpuSetInformation` passed as `0`; `HANDLE` is `*mut c_void` in windows-sys 0.59. Confirmed by cross-target check (2 × E0308). | Pass `null_mut()` (both call sites). |
| F2 | Rust/Windows | soundness (UB) | CPU-set blob held in `Vec<u8>` and iterated by casting to `&SYSTEM_CPU_SET_INFORMATION` — the struct needs 8-byte alignment (`u64 AllocationTag`); a byte Vec guarantees none. C was fine (`malloc` aligns). | 8-byte-aligned backing buffer (`Vec<u64>`) + `ptr::read_unaligned` + explicit bounds guards before touching `Size`/`Type`/payload. Same guards added to the C iterator. |
| F3 | Rust/Linux | semantic divergence | `topology()` used `Path::exists()` where C uses `fopen()` readability — differs when the sysfs marker exists but is unreadable. | `File::open(...).is_ok()`, matching C. |
| F4a | both parsers | divergences | On malformed input, C and Rust disagreed: `"0-3junk"` C-OK/Rust-ERR, `"5,,6"` C-ERR/Rust-OK, `"5 , 6"` C silently truncated to `[5]`, `"-1"` C emitted a **negative CPU id**, `","` C-ERR/Rust-OK. | One strict grammar (below) enforced identically in both. |
| F4b | Rust parser | DoS defect | `"0-999999999999"` made Rust materialize the whole range (`out.extend(a..=b)`) — OOM/hang. | Value cap `MAX_CPU_ID = 8191` in both implementations. |
| F4c | C parser | UB | `"18446744073709551616"`: strtol clamps to `LONG_MAX`, then `cpu++` in the emit loop signed-overflows (UB) before erroring via buffer exhaustion. | The 8191 cap rejects before any loop runs. |
| F5 | Rust API | documented choice | C reports errors as `-1`; Rust collapses them to an empty `Vec`. Callers (`PcoreHasher`) treat empty as "fall back to all CPUs". | Documented; intentional API difference, not a divergence in detection results. |
| F6 | C/Windows | residual risk | The C Windows branch has never been compiled here (no mingw toolchain installed). | Rust crate is the go-forward implementation and its Windows path is typecheck-verified; C branch mirrors it line-for-line. See checklist below. |

## The shared cpulist grammar

Both implementations enforce, byte-identically (Rust:
`parse_cpu_list`/`MAX_CPU_ID`; C: `parse_cpu_list`/`PCORE_MAX_CPU_ID`):

```
list := "" | term ("," term)* [","]      (one trailing comma tolerated)
term := num | num "-" num                (low <= high)
num  := [0-9]+                           (value <= 8191)
```

Everything else errors: whitespace anywhere, signs, trailing garbage,
empty middle terms, inverted ranges, values > 8191 (Linux's largest
`NR_CPUS` is 8192). Callers strip the sysfs trailing newline; the parsers
never trim.

## Differential corpus result

28 cases (canonical sysfs forms, tolerated legacy forms, and every
divergence found in layer 1/3). After the fixes: **C and Rust produce
identical results on all 28**, including `"0-8191"` → exactly 8192 CPUs on
both sides. The corpus is permanent:

- Rust: `affinity::tests::parser_matches_c_reference_corpus` (in-crate).
- C: `parse_harness.c` (kept with the C library's repo) run over the same
  inputs.

**Lock-step rule:** any change to either parser must re-run both and keep
all cases identical.

## Verification matrix

| Aspect | Status |
|---|---|
| Linux detection vs C vs sysfs ground truth (hybrid i9: P=0-11, E=12-19) | ✅ identical at runtime |
| Linux pin → `sched_getcpu()` readback | ✅ verified (in-crate test) |
| Parser: 28-case differential corpus | ✅ identical |
| Detection invariants (P∩E=∅, hybrid⇔E≠∅) | ✅ in-crate test |
| Windows module typecheck + clippy (`x86_64-pc-windows-gnu`) | ✅ clean |
| Windows runtime, homogeneous machine | ✅ CI `windows-latest` run #1: builds, tests pass, `--info` correctly reports 4-CPU homogeneous topology |
| Windows runtime, **hybrid** machine | ⏳ needs the Windows VM — checklist below |
| C Windows branch compile | ⏳ no mingw here; mirrors the verified Rust logic |

## Windows-VM verification checklist (when access returns)

1. `cargo test --release` and `cargo run --release -- --info` on the VM.
2. On hybrid hardware: confirm P/E lists against Task Manager / a
   known-good tool (e.g. Windows' own scheduling of a pinned busy loop).
3. Pin readback: `SetThreadAffinityMask` → `GetCurrentProcessorNumber()`.
4. If the VM has >64 logical CPUs (unlikely): expect the documented
   single-processor-group limitation to surface; ids ≥ 64 must error.
5. Optionally compile the C branch with MSVC/mingw and re-run the
   detection comparison C-vs-Rust on Windows.
