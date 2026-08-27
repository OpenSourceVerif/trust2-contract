# trust2-contract

Contract-based deductive verification for Rust.

`trust2-contract` lets you attach preconditions, postconditions, type invariants,
assertions, and assumptions to Rust code. `cargo verify` translates the annotated
Rust crate to Charon LLBC, converts LLBC to WhyML, and asks Why3 and an external
automatic prover to discharge the generated verification conditions.

```text
Rust + contracts -> Charon LLBC -> WhyML -> Why3 -> automatic prover
```

Contracts are verification annotations, not runtime checks. In an ordinary Cargo
build, the contract attributes are erased. Run `cargo verify` to check them.

## Example

```rs
use trust2_contract::prelude::*;

#[postcondition(|c| c >= a)]
#[postcondition(|c| c >= b)]
fn max(a: i32, b: i32) -> i32 {
    if a > b { a } else { b }
}

#[precondition(a >= b)]
#[postcondition(|c| c + b == a)]
fn strict_sub(a: u32, b: u32) -> u32 {
    a - b
}

#[precondition(x < 16)]
#[postcondition(|x2| x2 >= x)]
fn square(x: u8) -> u8 {
    x * x
}

fn main(){
}
```

Example verification result:

```text
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

## Prerequisites

### 1. Rust and rustup

Install [rustup](https://rustup.rs/). This repository pins the required nightly
toolchain and components in `rust-toolchain.toml`. Cargo/rustup will install them
when needed.

The same file must also be present at the root of every Cargo package or workspace
you want to verify; see [Set up a target project](#set-up-a-target-project).

### 2. Why3

The verifier starts the `why3` executable directly, so it must be installed and
available on `PATH`. This project is tested with **Why3 1.8.2**.

One way to install the tested version with [opam](https://opam.ocaml.org/) is:

```sh
opam update
opam install "why3=1.8.2"
eval "$(opam env)"
why3 --version
```

The Rust dependency `why3-ptree` and the `.mlw` files embedded in this repository
do not replace the Why3 executable. No Why3 IDE package is required.

### 3. At least one automatic prover

Why3 delegates proof obligations to an external prover. Install at least one of:

- [Alt-Ergo](https://alt-ergo.ocamlpro.com/)
- [CVC4](https://cvc4.github.io/downloads.html)
- [cvc5](https://cvc5.github.io/)
- [Z3](https://github.com/Z3Prover/z3)

You do not need to install all four. For example, Alt-Ergo can be installed in the
same opam switch as Why3:

```sh
opam install "alt-ergo=2.6.2"
eval "$(opam env)"
```

Alternatively, install Z3 or cvc5 using your operating system's package manager or
the solver's official installation instructions.

After installing or upgrading a prover, register it with Why3:

```sh
why3 config detect
why3 config list-provers
```

Do not continue until `list-provers` shows at least one of `Alt-Ergo`, `CVC4`,
`CVC5`, or `Z3`. Why3 may reject a solver version that it does not recognize. See
the [Why3 prover configuration documentation](https://why3.org/doc/manpages.html#the-config-command)
and `why3 config list-supported-provers` if detection fails.

## Build and verify with Make

The repository includes a `Makefile` that automates the project-specific setup
described above. Install `rustup` and `opam`, and create or select an opam switch,
before running it.

Install the Rust components pinned by `rust-toolchain.toml`, Why3 1.8.2,
Alt-Ergo 2.6.2, and the local `cargo-verify` binaries:

```sh
make build
```

Verify the independent `examples/readme` package. It consumes `trust2-contract`
through a path dependency, just like the target-project setup below:

```sh
make verify
```

Run `make help` to list the available commands. The package and prover versions
can be overridden when needed, for example:

```sh
make build WHY3_VERSION=1.8.2 ALT_ERGO_VERSION=2.6.2
make verify PROVER=Alt-Ergo,2.6.2
```

## Install `cargo verify`

The following is the equivalent manual installation method if you do not want to
use the `Makefile`.

From the root of this repository:

```sh
cargo install --path ./cargo-verify --locked
cargo verify --help
```

This installs `cargo-verify` and its Charon driver in Cargo's binary directory,
which normally is already on `PATH` when Rust was installed with rustup.

## Set up a target project

Assume this repository was cloned to `/path/to/trust2-contract` and the project to
verify is `/path/to/my-project`.

### 1. Use the pinned Rust toolchain

Copy the toolchain file to the root of the target Cargo package or workspace:

```sh
cd /path/to/my-project
cp /path/to/trust2-contract/rust-toolchain.toml ./rust-toolchain.toml
```

### 2. Add the contract package

Add `trust2-contract` as a path dependency:

```sh
cargo add trust2-contract \
  --path /path/to/trust2-contract/trust2-contract
```

eg:
```bash
cargo new package
cd package
cp ../rust-toolchain.toml ./rust-toolchain.toml
cargo add --path <repo-root>/trust2-contract
```

### 3. Add contracts

Import the prelude and annotate the code as shown in the example above.

API documentation can be generated with:

```sh
cargo doc --package trust2-contract --no-deps --open
```

### 4. Verify

Select a prover reported by `why3 config list-provers`. For example:

```sh
cargo verify --prover Alt-Ergo,2.6.2
```

Run `cargo verify --help` for all options.

## Development

The following opam packages are for developing this repository. They are not
required merely to install or run `cargo verify`:

```sh
opam switch create . 5.3.0
opam install ocaml-lsp-server odoc ocamlformat.0.27.0 utop
```

Set up repository maintenance remotes and hooks with:

```sh
git config set core.hooksPath .git-hooks

git remote add charon https://github.com/AeneasVerif/charon.git
git remote set-url charon --push no-push

git remote add proc-macro-crate https://github.com/bkchr/proc-macro-crate.git
git remote set-url proc-macro-crate --push no-push
```

### Charon

If Charon depends on the `openssl` crate, additional system packages may be
required. See the [`openssl` crate documentation](https://docs.rs/openssl/latest/openssl/#automatic).

This project patches Charon without bumping its upstream version.
