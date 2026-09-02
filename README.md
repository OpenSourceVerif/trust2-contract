# trust2-contract
Contract-based design and static verification for Rust

trust2-contract lets you attach preconditions, postconditions, type invariants, assertions, and assumptions to Rust code.

Contracts are verified statically. In an ordinary build, contract annotations are simply removed.

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
- Linux ARM64
- macOS AMD64
- macOS ARM64

## Requirements

1. [opam](https://opam.ocaml.org/)

2. [Why3find](https://git.frama-c.com/pub/why3find) at d194f55801a3bf6c76eb835b4cf18897c565470c

    1. Clone the Why3find repository at the specified commit.

        - For Git 2.49.0 or later:

            ```sh
            git clone https://git.frama-c.com/pub/why3find.git --revision d194f55801a3bf6c76eb835b4cf18897c565470c
            ```

        - For Git prior to 2.49.0:

            ```sh
            git init why3find
            cd why3find
            git remote add origin https://git.frama-c.com/pub/why3find.git
            git fetch origin d194f55801a3bf6c76eb835b4cf18897c565470c --depth 1
            git switch --detach FETCH_HEAD
            ```

    2. From the root of the Why3find repository, install Why3find with:

        ```sh
        opam install .
        ```

    3. Ensure that the `why3find` executable can be accessed where our program is run.

        See [Install OCaml](https://ocaml.org/install#linux_mac_bsd:~:text=Activate%20the%20opam,exec%20%2D%2D%20%5BCMD%5D) for information on how to access executables installed inside an opam switch.

    4. Optionally, remove the Why3find repository after installation.

## Installation

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
