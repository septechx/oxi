# Deprecated modules

The `src/codegen` module is deprecated

# Useful commands

- Run test suite: `just test`
- Run lint: `just lint`. If formatting issues are reported, fix them with `cargo fmt`
- Run compiler: `just run`. If the file is not named `main.oxi`, you will need to pass the `--entrypoint` flag. E.g. `just run x.oxi --entrypoint x`

# Language syntax

## Arrays

```
let a = []u8{1, 2, 3};
```

## Functions

```
fn add(a: u32, b: u32) u32 {
    return a + b;
}
```

The tests (`tests/integration.rs`) contain more example code
