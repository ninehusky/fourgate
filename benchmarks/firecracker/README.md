# Firecracker

This benchmark is the actual VMM executable distributed by Firecracker, not a
shim that artificially retains library code. The pinned upstream source is
recorded in `benchmark.toml`; no proof changes are applied.

Build in the provided Debian AArch64 Linux environment with Git, Rust 1.97.0, the
`aarch64-unknown-linux-musl` Rust target, `musl-gcc`, Clang, CMake, kernel headers,
and static musl libseccomp v2.6.1 (matching upstream's development container).
`Dockerfile` provides this environment; build it with `docker build --platform
linux/arm64 -t fourgate-firecracker benchmarks/firecracker`.
The pinned ARM64 Rust container image is also recorded in the manifest.
This checkpoint selects AArch64; it makes no claim about x86_64 counts.

```sh
git clone https://github.com/firecracker-microvm/firecracker.git /work/firecracker
git -C /work/firecracker checkout --detach dd7c24b0d165219c9d64f58ecedd0b0328b44fa0
rustup target add --toolchain 1.97.0 aarch64-unknown-linux-musl
cargo build --locked
FOURGATE=/absolute/fourgate/target/debug/fourgate \
SOURCE=/work/firecracker OUTPUT=/work/firecracker-baseline \
    bash benchmarks/firecracker/build.sh
```

The script refuses dirty source or a different revision and captures the Cargo
invocation/full logs with `record`. Output includes `firecracker`, SHA-256,
compiler versions, and the exact recipe. It preserves upstream release settings
(abort-on-panic, LTO, one codegen unit, default features). The additional
compiler flag adds DWARF debug information for attribution; do not charge debug
sections as executable size. The binary is not stripped. Do not set
`CARGO_PROFILE_RELEASE_DEBUG`: upstream's build script uses Cargo's `DEBUG`
variable to decide whether to substitute empty seccomp filters. The recipe
adds `-Cdebuginfo=2` through `RUSTFLAGS` while retaining `DEBUG=false`.

Then run the deterministic measurement stage (Linux or macOS with LLVM):

```sh
fourgate measure-panics --binary /work/firecracker-baseline/firecracker \
    --output /work/firecracker-panics
```

Building and disassembling do not require `/dev/kvm`. Running a guest does;
this checkpoint checks the executable's version and retained machine code,
not VM boot or workload behavior.

Upstream documents downloading and building this same executable in its
[getting-started guide](https://github.com/firecracker-microvm/firecracker/blob/dd7c24b0d165219c9d64f58ecedd0b0328b44fa0/docs/getting-started.md#getting-a-firecracker-binary).
