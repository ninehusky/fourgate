# Source → expected JSON

Start with a Rust file and its adjacent JSON:

| Code | Expected JSON | Panic in `before` |
|---|---|---|
| [bounds.rs](bounds.rs) | [bounds.json](bounds.json) | Index may exceed the array length |
| [unwrap.rs](unwrap.rs) | [unwrap.json](unwrap.json) | Option may be None |
| [arithmetic.rs](arithmetic.rs) | [arithmetic.json](arithmetic.json) | Divisor may be zero |
| [slice_copy.rs](slice_copy.rs) | [slice_copy.json](slice_copy.json) | Slice lengths may differ |
| [explicit.rs](explicit.rs) | [explicit.json](explicit.json) | Explicit panic on invalid input |

Each file has a `before` with one panic site and an `after` with none.
The expected `sites` array therefore contains only `before`. A missing site,
an extra site, or a panic appearing in `after` fails the comparison.

These are small **views of the actual `panics.json`**, not full-binary snapshots.
The comparison keeps the function name, panic kind, destination symbol, and
fixture source frames/lines. It excludes unrelated runtime/library sites and
omits addresses, instruction text, column numbers, path prefixes, and aggregate
statistics. Nothing changes in the CLI's full output.

CI compiles these files with Rust 1.97.0, runs `measure-panics` on the resulting
ELF binaries, and compares against the checked-in JSON. There is no snapshot
update command: changes to the source or expected output are reviewed together.

Run on Linux with LLVM installed:

```sh
cargo test --locked --test measure_panics
```

`runtime.rs` supplies the shared entry point and panic handler. The binaries
are disassembled, never executed; `no_std` avoids pulling in a whole application.
