run *args:
    cargo run --bin pge-cost -- {{args}}

run-ai *args:
    AGENT=1 cargo run --bin pge-cost -- {{args}}

build:
    cargo build --release --bin pge-cost
    mkdir -p bin
    cp target/release/pge-cost bin/pge-cost

test:
    cargo test --all-targets

lint:
    rustfmt --check --edition 2024 --config skip_children=true src/lib.rs src/model.rs src/calculator.rs src/spreadsheet.rs src/cli.rs src/main.rs tests/calculator.rs tests/spreadsheet.rs tests/cli.rs
    cargo clippy --all-targets -- -D warnings

coverage:
    cargo llvm-cov --all-targets --fail-under-lines 90

check: lint test coverage
