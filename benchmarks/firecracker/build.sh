#!/usr/bin/env bash
# Run on native AArch64 Linux. FOURGATE, SOURCE, OUTPUT must be absolute paths.
set -euo pipefail
: "${FOURGATE:?path to the fourgate executable}"
: "${SOURCE:?path to the pinned Firecracker checkout}"
: "${OUTPUT:?new benchmark output directory}"

RECIPE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
get_pin() { sed -n "s/^$1 = \"\(.*\)\"$/\1/p" "$RECIPE/benchmark.toml"; }
REVISION=$(get_pin revision)
TOOLCHAIN=$(get_pin toolchain)
TARGET=$(get_pin target)
LIBSECCOMP_REVISION=$(get_pin libseccomp_revision)
test "$(uname -sm)" = "Linux aarch64" || { echo 'requires native AArch64 Linux' >&2; exit 1; }
test "$(git -C "$SOURCE" rev-parse HEAD)" = "$REVISION"
test -z "$(git -C "$SOURCE" status --porcelain)" || { echo 'source checkout must be clean' >&2; exit 1; }
test "$(cat /usr/local/lib/libseccomp-revision.txt)" = "$LIBSECCOMP_REVISION"
mkdir "$OUTPUT"
cp "$RECIPE/benchmark.toml" "$OUTPUT/benchmark.toml"
cp "$RECIPE/build.sh" "$OUTPUT/build.sh"
rustc +"$TOOLCHAIN" -vV > "$OUTPUT/rustc.txt"
cargo +"$TOOLCHAIN" -V > "$OUTPUT/cargo.txt"
dpkg-query -W > "$OUTPUT/builder-packages.txt"
cp /usr/local/lib/libseccomp-revision.txt "$OUTPUT/libseccomp-revision.txt"
"$FOURGATE" record --repo "$SOURCE" --output "$OUTPUT/build" -- \
    env RUSTFLAGS='-Ccodegen-units=1 -Cdebuginfo=2' \
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc \
    CC_aarch64_unknown_linux_musl=musl-gcc \
    cargo +"$TOOLCHAIN" build --locked --release --target "$TARGET" \
    --target-dir "$OUTPUT/target" -p firecracker --bin firecracker
if grep -Eq 'Using empty default seccomp policy|No default seccomp policy' "$OUTPUT/build/stdout.log" "$OUTPUT/build/stderr.log"; then
    echo 'refusing a build with substituted seccomp policy' >&2
    exit 1
fi
find "$OUTPUT/target/$TARGET/release/build" -name seccomp_filter.bpf \
    -exec sha256sum {} \; > "$OUTPUT/seccomp-filter.sha256"
find "$OUTPUT/target/$TARGET/release/build" -name seccomp_filter.bpf \
    -exec wc -c {} \; > "$OUTPUT/seccomp-filter-size.txt"
cp "$OUTPUT/target/$TARGET/release/firecracker" "$OUTPUT/firecracker"
sha256sum "$OUTPUT/firecracker" > "$OUTPUT/binary.sha256"
"$OUTPUT/firecracker" --version > "$OUTPUT/version.txt"
printf '%s\n' "$OUTPUT/firecracker"
