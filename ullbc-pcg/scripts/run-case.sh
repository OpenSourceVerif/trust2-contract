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
  cargo +"$toolchain" build --locked --manifest-path charon/charon/Cargo.toml --bin charon
  cargo +"$toolchain" build --locked -p ullbc-pcg --features driver --bin ullbc-pcg-driver
  target_dir() {
    cargo +"$toolchain" metadata --no-deps --format-version 1 --manifest-path "$1" |
      python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])'
  }
  charon_target=$(target_dir charon/charon/Cargo.toml)
  analysis_target=$(target_dir Cargo.toml)
  mkdir -p "$analysis_target/pcg-viewer/ullbc"
  run_dir=$(mktemp -d "$analysis_target/pcg-viewer/ullbc/case.XXXXXX")
  trap 'rm -rf "$run_dir"' EXIT
  CHARON_DRIVER="$analysis_target/debug/ullbc-pcg-driver" \
  ULLBC_PCG_OUTPUT="$run_dir/viewer" ULLBC_PCG_FUNCTION=mutable_borrow \
    "$charon_target/debug/charon" rustc --export-borrowck --mir promoted --sysroot default \
    --error-on-warnings --dest-file "$run_dir/case.llbc" -- --crate-type lib \
    --crate-name ullbc_pcg_case ullbc-pcg/tests/fixtures/mutable_borrow.rs
  mkdir -p "$package_dir/visualization/data"
  cp "$run_dir/viewer/"*.dot "$package_dir/visualization/data/"
  cp "$run_dir/viewer/pcg.json" "$package_dir/visualization/data/pcg.json"
  cp "$run_dir/viewer/analysis.json" "$package_dir/visualization/data/analysis.json"
else
  cargo +"$toolchain" run --locked -p ullbc-pcg -- \
    ullbc-pcg/tests/fixtures/mutable_borrow.ullbc --function mutable_borrow \
    --output ullbc-pcg/visualization/data
fi

echo "Data ready. In $package_dir/visualization run: pnpm install --frozen-lockfile && pnpm run build && pnpm run serve"
