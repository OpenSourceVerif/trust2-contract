# Installation

Build trust2-contract from source on Linux AMD64, macOS AMD64, or macOS ARM64. The steps below use a local OCaml environment and Alt-Ergo as the initial prover. Commands assume Bash or Zsh.

## 1. Prepare the system tools

Install Git, a C/C++ build toolchain, and opam.

On Debian or Ubuntu:

```sh
sudo apt update
sudo apt install git curl build-essential pkg-config libgmp-dev opam
```

On macOS, install the Command Line Tools if they are not already available, then install opam using [Homebrew](https://brew.sh/):

```sh
xcode-select --install
brew install git opam pkg-config
```

For other distributions, follow the [opam installation instructions](https://opam.ocaml.org/doc/Install.html). Opam may request additional system packages when resolving dependencies.

## 2. Get the source

```sh
git clone https://github.com/OpenSourceVerif/trust2-contract.git
cd trust2-contract
```

This is the repository root: it contains `Cargo.toml`, `rust-toolchain.toml`, and `trust2-contract.opam`. Run the remaining setup commands here unless another directory is specified. Keep this checkout: the contract library is currently used as a path dependency.

## 3. Set up Rust

If rustup is not installed, use the [official Rust installer](https://rust-lang.org/tools/install/):

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
. "$HOME/.cargo/env"
```

From the repository root, check the selected toolchain:

```sh
rustc --version
rustup show active-toolchain
```

Rustup uses the repository's `rust-toolchain.toml` and downloads its toolchain and requested components when needed. The current file pins `nightly-2026-06-01` with `rustc-dev`, `llvm-tools`, and `miri`. Use the file shipped with your checkout; the frontend depends on Rust compiler internals.

## 4. Set up the OCaml environment

If this is your first use of opam, initialize it without creating a global switch:

```sh
opam init --bare
```

Create a local switch with the OCaml version used by the repository's development setup, activate it, and install the project dependencies:

```sh
opam switch create . 5.3.0
eval $(opam env)
opam install . --deps-only
```

If this checkout already has a local switch, reuse it instead of creating it again. The dependency manifest pins a particular development revision of Why3find; use `opam install . --deps-only` to follow that pin.

Check that the environment exposes Why3find:

```sh
command -v why3find
```

In a new terminal, return to the repository root and run `eval $(opam env)` before invoking the verifier. This puts the local switch's executables on `PATH`, including when you later enter an example directory. See [opam usage](https://opam.ocaml.org/doc/Usage.html) for switch management.

## 5. Install a prover

Verification needs an external prover as well as Why3find. To start with [Alt-Ergo](https://opam.ocaml.org/packages/alt-ergo/), install it in the same switch and let Why3 detect it:

```sh
opam install alt-ergo
why3 config detect
why3 config list-provers
```

Confirm that Alt-Ergo appears in the detected provers. Repeat detection after installing another prover, as described in the [Why3 configuration documentation](https://why3.org/doc/manpages.html#the-config-command).

The verifier's default prover list is Alt-Ergo, CVC5, and Z3. The example below explicitly selects Alt-Ergo so that it only requests the prover installed in this walkthrough. Additional provers can be selected with `--prover`, for example `--prover alt-ergo,z3` once both are installed and detected.

## 6. Install the verifier

From the repository root:

```sh
cargo install --path ./cargo-verify
cargo verify --help
```

Cargo normally installs `cargo-verify` in `~/.cargo/bin`. Keep that directory on `PATH` together with the active opam switch's `bin` directory.

## 7. Check the installation

Run the example included in the repository:

```sh
cd examples/readme
cargo verify --prover alt-ergo
```

This checks the Rust frontend, specification translation, and prover invocation together. A successful run should finish with a `Verified` summary and exit successfully; displaying `--help` alone only checks that the command is installed.

Continue with [Basic Usage](basic-usage.md) to set up your own package.

## Troubleshooting

- **`cargo verify` is not found:** check that Cargo's installation directory is on `PATH`; with the default rustup setup, reload `. "$HOME/.cargo/env"`.
- **`why3find` is not found:** activate the local switch from the repository root with `eval $(opam env)` in the terminal that runs verification.
- **No matching prover:** check `why3 config list-provers`, ensure the prover executable is on `PATH`, and rerun `why3 config detect`.
- **Rust compiler or component mismatch:** run from a checkout using its pinned `rust-toolchain.toml`; do not substitute a different nightly.
