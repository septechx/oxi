export RUSTFLAGS := "-Clinker=clang -Clink-args=--ld-path=mold"

precommit: lint test

build MODE="release":
    cargo build {{ if MODE == "release" { "--release" } else { "" } }}

run *ARGS:
    env OXI_ROOT="$(pwd)" RUST_BACKTRACE=1 cargo run -- {{ARGS}}

test FILTER="":
    env OXI_ROOT="$(pwd)" cargo test {{FILTER}}

check:
    cargo check

clean:
    cargo clean

install PREFIX="/usr": (build "release")
    sudo install -D -m755 target/release/oxic {{PREFIX}}/bin/oxic
    sudo rsync -a --delete lib/oxi/ {{PREFIX}}/lib/oxi

lint:
    cargo clippy --all-targets --all-features -- -Dwarnings
    cargo fmt -- --check
