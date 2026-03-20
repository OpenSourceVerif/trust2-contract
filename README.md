# trust2-contract
Contract-based design for Rust


## Quick Start

1. After cloning this repository, first create a new package.

```bash
cargo new package
cd package
cargo add --path <repo-root>/trust2-contract
```
Copy the test example into src/main.rs or src/lib.rs.
```Rust
//for example
use trust2_contract::{invariant, postcondition, precondition};

#[expect(dead_code)]
#[precondition(x < 16)]
#[postcondition(|x2| x2 >= x)]
fn square(x: u8) -> u8 {
    x * x
}

#[expect(dead_code)]
#[invariant(self.start <= self.end)]
struct RefRange<'a, T: PartialOrd> {
    start: &'a T,
    end: &'a T,
}

```

2. Set up the toolchain.

Copy the rust-toolchain file from this repository to package/, then run:

```bash
rustup toolchain install
```

3. Install cargo-verify.

```bash
cargo install --path <repo-root>/cargo-verify
```

4. Generate llbc with contracts.
```bash
cargo verify --charon-out-dir Charon-LLBC --charon-pretty-print > output.llbc.pretty
```

## Build

The following commands assume the current working directory is the root of this repository.

1. Prerequisites: rustup, opam, dune.

2. `rustup toolchain install`

3. Ensure the following packages are installed: pkg-config, libgmp-dev.

4. `sh build.sh` or `sh build.sh --install`

## Usage

- Copy `./rust-toolchain.toml` to your package/workspace root.

- To specify contracts in source code, use `trust2-contract` package.

  Run `cargo doc --package trust2-contract --no-deps --open` for documentation.

  Add `./trust2-contract` as a path dependency to use.

- To verify contracts specified, use `cargo verify` subcommand.

  Run `cargo verify --help` for help.

## Development setup

```sh
git config set core.hooksPath .git-hooks

git remote add charon https://github.com/AeneasVerif/charon.git
git remote set-url charon --push no-push

git remote add proc-macro-crate https://github.com/bkchr/proc-macro-crate.git
git remote set-url proc-macro-crate --push no-push
```

## About Charon modification

Charon version is not bumped.
