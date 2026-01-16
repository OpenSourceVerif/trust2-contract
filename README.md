# trust2-contract
Contract-based design for Rust

## Usage

- To specify contracts in source code, use `trust2-contract` package.

  Run `cargo doc --package trust2-contract --no-deps --open` for documentation.

  Add `./trust2-contract` as a path dependency to use.

- To verify contracts specified, use `cargo verify` subcommand.

  Install through `cargo install --path ./cargo-verify`.

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

Charon ML library is not updated accordingly.
