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

Inventory schema version 3 embeds `enclosing_function: {name, address}` on each
site (or null when unavailable). There is no top-level function table. For
aliases, the name is the first sorted symbol name at that function's address.
Fixed analysis caveats live in this document, not a JSON `limitations` field.

`PanicKind` distinguishes bounds checks, slice ranges, slice length mismatches,
unwrap/expect, arithmetic, borrow violations, assertion helpers, generic panic
messages, and runtime entry points. Generic panic messages cannot distinguish
`panic!`, `assert!`, and `unreachable!`. Attribution is explicitly located,
symbols-only, or unavailable; it is never inferred from missing diagnostics.

A site is a direct call or branch into a recognized panic symbol from outside
recognized panic machinery. Calls between panic helpers are excluded. Aliases
at one address count once. The same source expression can produce several sites
after inlining, or none after optimization.

This is a partial static inventory, not a proof of panic freedom. Indirect
transfers are counted as unresolved. Unknown wrappers and fully inlined panic
machinery can hide additional sites. Zero detected sites is inconclusive. Crate
attribution uses source paths, not Cargo package IDs. This checkpoint does not
measure byte savings or transitively panic-free caller regions.
Counts describe retained machine code, not runtime frequency or reachability
from a workload.

## Build provenance

Preserve build commands and logs with `fourgate record`; analyzer metadata
identifies the binary but does not reconstruct its build configuration.

## Verification

The executable fixture deliberately retains known bounds, unwrap, divide,
copy-from-slice, and explicit-panic sites. On Linux with LLVM installed:

```sh
cargo test --locked --test measure_panics -- --ignored
```

The test compiles real optimized machine code, checks each expected panic family
and source function, verifies assembly references, and checks overwrite refusal.
