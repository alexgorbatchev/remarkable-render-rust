default:
    @just --list

build:
    cargo build --all-targets

test:
    cargo test

fmt:
    cargo fmt --package remarkable-render -- --check

lint:
    cargo clippy --all-targets -- -D warnings

coverage:
    cargo llvm-cov --all-targets --fail-under-lines 90

check: fmt lint test coverage
