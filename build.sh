#!/bin/sh

set -e

cargo build -p cargo-verify --release

(
    cd trust2-contract-verifier
    dune pkg lock
    dune build -p trust2-contract-verifier @install
)
