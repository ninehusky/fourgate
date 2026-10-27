# Measure panics

Deterministic Rust stage; no agent is needed to run the analysis.

Input: a linked, unstripped x86_64 or AArch64 ELF executable. Embedded DWARF
enables source and inline attribution. LLVM's `llvm-objdump` and
`llvm-symbolizer` must be on PATH, or supplied with `--llvm-tools /path/to/bin`.

```sh
fourgate measure-panics --binary /path/to/binary --output /path/to/new-report
```

The output directory's parent must exist. Existing output is never overwritten.

- `disassembly.asm`: exact LLVM disassembly, with virtual instruction addresses.
- `panics.json`: typed inventory, per-site enclosing function name/address, panic family,
  direct destination, one-based assembly line, inline frames, source-path crate
  attribution, counts, and coverage statistics.
- `summary.md`: counts by family and source-path crate, plus unresolved cases.
- `metadata.json`: binary SHA-256, tool versions, classifier/schema versions,
  and exact tool/CLI arguments. Written last; absent on an incomplete run.

Inventory schema version 4 embeds `enclosing_function: {name, address}` on each
site (or null when unavailable). There is no top-level function table. For
aliases, the name is the first sorted symbol name at that function's address.
Fixed analysis caveats live in this document, not a JSON `limitations` field.

Compiler-check kinds follow Rust 1.97's MIR
[`AssertKind::panic_function`](https://github.com/rust-lang/rust/blob/1.97.0/compiler/rustc_middle/src/mir/terminator.rs)
and [core panic lang items](https://github.com/rust-lang/rust/blob/1.97.0/library/core/src/panicking.rs).
Bounds, divide-by-zero, remainder-by-zero, each overflow operation, pointer checks,
invalid enum construction, and coroutine resume checks are distinct. MIR operands
are not recovered; `target_symbol` retains the precise runtime helper, including
coroutine flavor. Unknown `panic_const` helpers are `unknown_compiler_assert`.

Library failures (slice operations, unwrap/expect, borrows) have separate kinds:
MIR's enum does not categorize every library panic. Core's own `AssertKind`
only distinguishes Eq/Ne/Match assertion macros, not all panic origins.
Generic panic-message destinations cannot distinguish `panic!`, `assert!`, and
`unreachable!`. Attribution is explicitly located,
symbols-only, or unavailable; it is never inferred from missing diagnostics.

A site is a direct call or branch into a recognized panic symbol from outside
recognized panic machinery. Calls between panic helpers are excluded. Aliases
at one address count once. The same source expression can produce several sites
after inlining, or none after optimization.

This is a partial static inventory, not a proof of panic freedom. Indirect
transfers are counted as unresolved. Unknown wrappers and fully inlined panic
machinery can hide additional sites. `sites: []` means no caller-to-sink direct
panic sites were counted, not that panic machinery is absent. `recognized_sinks`
counts recognized panic entry points separately; it can be nonzero with no sites.
Zero detected sites is therefore inconclusive. Crate
attribution uses source paths, not Cargo package IDs. This checkpoint does not
measure byte savings or transitively panic-free caller regions.
Counts describe retained machine code, not runtime frequency or reachability
from a workload.

## Build provenance

Preserve build commands and logs with `fourgate record`; analyzer metadata
identifies the binary but does not reconstruct its build configuration.

## Verification

Five small [Rust/expected-JSON pairs](../tests/fixtures/panics/README.md) document
bounds, unwrap, divide, slice-copy, and explicit-panic sites. Each includes a
`before` with a panic and an `after` without one. On Linux with LLVM installed:

```sh
cargo test --locked --test measure_panics
```

The test compiles real optimized machine code and compares the fixture-owned
sites with the adjacent expected JSON. Missing or unexpected sites fail CI.
