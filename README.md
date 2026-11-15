# trust2-contract
Contract-based design and static verification for Rust

trust2-contract lets you attach preconditions, postconditions, type invariants, assertions, and assumptions to Rust code.

Contracts are verified statically. In an ordinary build, contract annotations are simply removed.

## Manual

Research workspace: [Open the website](sites/index.html) · [Website README](sites/README.md).
Open `sites/index.html` in a local browser. The website source and pages live in this repository under `sites/`.

Read the user guide and specification reference sources: [English](docs/specification-reference/src/en/introduction.md) · [简体中文](docs/specification-reference/src/zh/introduction.md).

To build and browse the full HTML manual locally (`docs/specification-reference/book/index.html`) or download the built artifact from CI, see [the manual README](docs/specification-reference/README.md).

## Example

```rs
use trust2_contract::prelude::*;

#[postcondition(|c| c >= a)]
#[postcondition(|c| c >= b)]
pub fn max(a: i32, b: i32) -> i32 {
    if a > b { a } else { b }
}

#[precondition(a >= b)]
#[postcondition(|c| c + b == a)]
pub fn strict_sub(a: u32, b: u32) -> u32 {
    a - b
}

#[precondition(x < 16)]
#[postcondition(|x2| x2 >= x)]
pub fn square(x: u8) -> u8 {
    x * x
}
```

Verification result:

```
Entering directory '/tmp/.tmpj8g1QI'
Theory trust2_contract_example.Top: ✔ (3)
Proved (trust2_contract_example.mlw) ✔
Leaving directory '/tmp/.tmpj8g1QI'
    Verified trust2_contract_example in 2.19s
```

## Platform Support

- Linux AMD64
- macOS AMD64
- macOS ARM64

## Requirements

- [opam](https://opam.ocaml.org/)

## Installation

```sh
opam install . --deps-only
```

Ensure that the installed `why3find` executable can be accessed where our verifier program is run.

See [Install OCaml](https://ocaml.org/install#linux_mac_bsd:~:text=Activate%20the%20opam,exec%20%2D%2D%20%5BCMD%5D) for information on how to access executables installed inside an opam switch.

```sh
cargo install --path ./cargo-verify
```

Note that this will automatically install the Rust toolchain pinned by `./rust-toolchain.toml`.

## Usage

1. (Recommended) Copy `./rust-toolchain.toml` to your Cargo package/workspace root.

2. To specify contracts in source code, use the `trust2-contract` package.

   Add `./trust2-contract` as a path dependency.

   Run `cargo doc --package trust2-contract --no-deps --open` for documentation.

3. To verify contracts specified, use the `cargo verify` subcommand.

   Run `cargo verify --help` for help.

## Development

```sh
opam switch create . 5.3.0
opam install ocaml-lsp-server odoc ocamlformat.0.27.0 utop
opam install . --deps-only
```

```sh
git config set core.hooksPath .git-hooks

git remote add charon https://github.com/AeneasVerif/charon.git
git remote set-url charon --push no-push

git remote add proc-macro-crate https://github.com/bkchr/proc-macro-crate.git
git remote set-url proc-macro-crate --push no-push
```

### Charon

If Charon depends on the openssl crate, additional steps may be required to build. See [openssl - Rust](https://docs.rs/openssl/latest/openssl/#automatic).

This project patches Charon, but does not bump its version.
