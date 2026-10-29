#!/usr/bin/env bash
set -euo pipefail

package_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
repo_dir=$(cd -- "$package_dir/.." && pwd)
toolchain=nightly-2026-06-01
cd "$repo_dir"

if [[ $# -gt 1 || (${1:-} != "" && ${1:-} != "--fixture") ]]; then
  echo "usage: $0 [--fixture]" >&2
  exit 2
fi

if [[ ${1:-} != --fixture ]]; then
  # Extraction only. No dependency on MIR-PCG or its analysis outputs.
  cargo +"$toolchain" build --locked --manifest-path charon/charon/Cargo.toml --bin charon --bin charon-driver
  charon_target=$(cargo +"$toolchain" metadata --no-deps --format-version 1 --manifest-path charon/charon/Cargo.toml |
    python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
  "$charon_target/debug/charon" rustc --ullbc --export-borrowck --mir promoted --sysroot default \
    --dest-file ullbc-pcg/tests/fixtures/mutable_borrow.ullbc -- \
    --crate-type lib --crate-name ullbc_pcg_case ullbc-pcg/tests/fixtures/mutable_borrow.rs
fi

cargo +"$toolchain" run --locked -p ullbc-pcg -- \
  ullbc-pcg/tests/fixtures/mutable_borrow.ullbc --function mutable_borrow \
  --output ullbc-pcg/visualization/data

echo "Data ready. In $package_dir/visualization run: pnpm install --frozen-lockfile && pnpm run build && pnpm run serve"
