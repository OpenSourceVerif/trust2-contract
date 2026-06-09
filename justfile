cov *args="test --test cargo trust2-contract-sample":
    cd charon/charon && cargo llvm-cov {{args}} --html
    cd charon/charon && cd target/llvm-cov/html && uv run python -m http.server 1234 --bind localhost

test:
    cd charon/charon && cargo test --test cargo trust2-contract-sample