# trust2-contract
Contract-based design for Rust

## Example

Source code:

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
File /tmp/.tmpMpQwkO/trust2_contract_example.mlw:
Goal max''_trust2_contract_example'vc.
Prover result is: Valid (0.02s, 190 steps).

File /tmp/.tmpMpQwkO/trust2_contract_example.mlw:
Goal strict_sub''_trust2_contract_example'vc.
Prover result is: Valid (0.04s, 450 steps).

File /tmp/.tmpMpQwkO/trust2_contract_example.mlw:
Goal square''_trust2_contract_example'vc.
Prover result is: Valid (0.04s, 302 steps).

    Verified trust2_contract_example in 0.76s
```

## Requirements

1. [Why3](https://why3.org/)

   This project is tested with Why3 1.8.2.

   Ensure that the `why3` command is added to PATH so that our program can call it.

2. Install and register external automatic provers for Why3.

   This project supports

   - [Alt-Ergo](https://alt-ergo.ocamlpro.com/),
   - [CVC4](https://cvc4.github.io/downloads.html),
   - [cvc5](https://cvc5.github.io/downloads.html), and
   - [Z3](https://github.com/Z3Prover/z3).

   Other provers may not work.

   To register provers in Why3, see https://why3.org/doc/manpages.html#the-config-command. The latest version of a prover may not be supported by Why3. You can check `provers-detection-data.conf` described in the link for supported versions.

## Installation

`cargo install --path ./cargo-verify`

## Usage

1. Copy `./rust-toolchain.toml` to your Cargo package/workspace root.

2. To specify contracts in source code, use the `trust2-contract` package.

   Add `./trust2-contract` as a path dependency.

   Run `cargo doc --package trust2-contract --no-deps --open` for documentation.

3. To verify contracts specified, use the `cargo verify` subcommand.

   Run `cargo verify --help` for help.

## Development

```sh
opam switch create . 5.3.0
opam install ocaml-lsp-server odoc ocamlformat.0.27.0 utop
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
