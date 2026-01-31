build MODE="release":
    cargo build {{ if MODE == "release" { "--release" } else { "" } }}

run *ARGS:
    env OXI_ROOT="$(pwd)" cargo run -- {{ARGS}}

test FILTER="":
    env OXI_ROOT="$(pwd)" cargo test {{FILTER}}

check:
    cargo check

clean:
    cargo clean

install PREFIX="/usr": build
    sudo install -D -m755 target/release/oxic {{PREFIX}}/bin/oxic
    sudo rsync -a --delete lib/oxi/ {{PREFIX}}/lib/oxi

lint:
    cargo clippy --all-targets --all-features -- -Dwarnings
