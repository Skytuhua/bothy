#!/bin/sh
# Builds the C library and runs tests/smoke.c against it (Linux, macOS).
set -eu
here=$(cd "$(dirname "$0")/.." && pwd)
root=$(cd "$here/../.." && pwd)
cargo build --release --locked -p bothy-ffi --manifest-path "$root/Cargo.toml"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
${CC:-cc} -std=c11 -Wall -Wextra -Werror -I"$here/include" "$here/tests/smoke.c" \
  -L"$root/target/release" -lbothy_ffi -o "$tmp/smoke"
LD_LIBRARY_PATH="$root/target/release" DYLD_LIBRARY_PATH="$root/target/release" "$tmp/smoke" "$tmp"
