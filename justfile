default:
    @just --list

check:
    cargo fmt --check
    cargo clippy --all-targets --locked -- -D warnings
    cargo test --locked

[positional-arguments]
run output:
    cargo run --locked -- "$1"
