# Deprecated modules

The `src/codegen` module is deprecated

# Useful commands

- Run test suite: `just test`
- Run lint: `just lint`. If formatting issues are reported, fix them with `cargo fmt`
- Run compiler: `just run <file>`.

# Language syntax

A Language grammar specification is available at `docs/grammar.txt`

## Arrays

```text
let a = [1, 2, 3];
```

## Functions

```text
fn add(a: u32, b: u32) u32 {
    a + b
}
```

The tests (`tests/integration.rs`) contain more example code
