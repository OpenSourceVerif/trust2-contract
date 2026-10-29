#!/usr/bin/env bash
set -euo pipefail
package_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$package_dir/.."
toolchain=nightly-2026-06-01
cargo +"$toolchain" build --locked --manifest-path charon/charon/Cargo.toml --bin charon --bin charon-driver
charon_target=$(cargo +"$toolchain" metadata --no-deps --format-version 1 --manifest-path charon/charon/Cargo.toml |
  python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
for name in mutable_borrow borrows cfg_borrows; do
  "$charon_target/debug/charon" rustc --ullbc --export-borrowck --mir promoted --sysroot default \
    --dest-file "ullbc-pcg/tests/fixtures/$name.ullbc" -- \
    --crate-type lib --crate-name "ullbc_pcg_$name" "ullbc-pcg/tests/fixtures/$name.rs"
done
