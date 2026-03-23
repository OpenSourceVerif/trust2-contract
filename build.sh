#!/bin/sh

set -e

cargo build -p cargo-verify --release

# Otherwise, dune reports broken symbolic links.
{
    mkdir -p charon/charon/target/doc/charon
    touch -a charon/charon/target/doc/charon/index.html
    mkdir -p charon/_build/default/_doc/_html/charon
    touch -a charon/_build/default/_doc/_html/charon/index.html
}
(
    cd trust2-contract-verifier
    dune pkg lock
    dune build --release
)
