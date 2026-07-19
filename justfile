export RUSTFLAGS := "-Clinker=clang -Clink-args=--ld-path=mold"

precommit: lint test

build MODE="release":
    cargo build {{ if MODE == "release" { "--release" } else { "" } }}

run *ARGS:
    env OXI_ROOT="$(pwd)" RUST_BACKTRACE=1 cargo run -- {{ARGS}}

run-test *ARGS:
    env OXI_ROOT="$(pwd)" RUST_BACKTRACE=1 cargo run -- tests/integration/{{ARGS}}

test FILTER="":
    env OXI_ROOT="$(pwd)" cargo test --workspace {{FILTER}}

check:
    cargo check

clean:
    cargo clean

install PREFIX="/usr": (build "release")
    sudo install -D -m755 target/release/oxic {{PREFIX}}/bin/oxic
    sudo rsync -a --delete lib/oxi/ {{PREFIX}}/lib/oxi

lint:
    cargo clippy --all-targets --all-features --workspace -- -Dwarnings
    cargo fmt -- --check
    cargo run --manifest-path crates/oxic_diag_lint/Cargo.toml -- src/

coverage FORMAT="Html":
    cargo tarpaulin --out {{FORMAT}}

coverage-open: (coverage "Html")
    xdg-open tarpaulin-report.html
