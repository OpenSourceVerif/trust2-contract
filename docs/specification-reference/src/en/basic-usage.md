# Basic Usage

After [installation](installation.md), copy the repository's `rust-toolchain.toml` into your package root and add `trust2-contract` to `Cargo.toml`:

```toml
[dependencies]
trust2-contract = { path = "/path/to/trust2-contract/trust2-contract" }
```

Replace the path with the location of the `trust2-contract` library inside your checkout.

## Write a contract

Import the contract annotations and attach them to a function:

```rust,ignore
use trust2_contract::prelude::*;

#[precondition(x < 16)]
#[postcondition(|result| result >= x)]
pub fn square(x: u8) -> u8 {
    x * x
}
```

The precondition requires callers to provide `x < 16`. The postcondition states that the returned value, named `result`, is at least `x`.

This function can be used in either a library or an executable.

## In a library

Place the code in `src/lib.rs`. From the package root, run:

```sh
cargo verify --prover alt-ergo -- --lib
```

The verifier checks the function under its precondition. No `main` function is needed.

## In an executable

Place the same code in `src/main.rs` and add an entry point:

```rust,ignore
fn main() {
    let _ = square(4);
}
```

Then run:

```sh
cargo verify --prover alt-ergo -- --bin my-app
```

Replace `my-app` with your binary target's name, which defaults to the package name in `Cargo.toml`. Here, `main` calls `square` with an argument that satisfies its precondition.

Both commands use the Alt-Ergo prover configured during installation. The arguments after `--` select the Cargo target: `--lib` for a library, or `--bin` for an executable. Run them from the package directory, including when the package is a workspace member.

To run an executable normally, use `cargo run`. Ordinary builds remove contract annotations; use `cargo verify` to check contracts statically.

See [Function Contracts](function-contracts.md) for more about preconditions and postconditions.
