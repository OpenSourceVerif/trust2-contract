SHELL := /bin/bash
.SHELLFLAGS := -eu -o pipefail -c
.DEFAULT_GOAL := help

WHY3_VERSION ?= 1.8.2
ALT_ERGO_VERSION ?= 2.6.2
PROVER ?= Alt-Ergo,$(ALT_ERGO_VERSION)

.PHONY: help build rust-env prover-env check-env verify

help:
	@printf '%s\n' \
		'make build   Install the pinned Rust components, Why3, Alt-Ergo, and cargo-verify' \
		'make verify  Verify the independent examples/readme package' \
		'' \
		'Optional: make verify PROVER=Z3,4.12.5'

# rustup and opam themselves are host prerequisites. Everything project-specific
# is pinned here to the versions documented in README.md.
rust-env:
	@command -v rustup >/dev/null || { echo 'error: rustup is required (https://rustup.rs)' >&2; exit 1; }
	@command -v cargo >/dev/null || { echo 'error: cargo is required (install it with rustup)' >&2; exit 1; }
	rustup component add rustc-dev llvm-tools

prover-env:
	@command -v opam >/dev/null || { echo 'error: opam is required (https://opam.ocaml.org)' >&2; exit 1; }
	@opam switch show >/dev/null 2>&1 || { echo 'error: create or select an opam switch first' >&2; exit 1; }
	opam install --yes "why3=$(WHY3_VERSION)" "alt-ergo=$(ALT_ERGO_VERSION)"
	opam exec -- why3 config detect

build: rust-env prover-env
	cargo install --path ./cargo-verify --locked --force
	@$(MAKE) --no-print-directory check-env

check-env:
	@command -v cargo-verify >/dev/null || { echo 'error: cargo-verify is missing; run `make build` first' >&2; exit 1; }
	@opam exec -- why3 --version
	@opam exec -- why3 config list-provers | grep -F 'Alt-Ergo $(ALT_ERGO_VERSION)' >/dev/null || { \
		echo 'error: Why3 did not detect Alt-Ergo $(ALT_ERGO_VERSION); run `make build` again' >&2; \
		exit 1; \
	}

verify: check-env
	cd examples/readme && opam exec -- cargo verify --prover "$(PROVER)"
