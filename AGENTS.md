# Deprecated modules

The `src/codegen` module is deprecated

# Useful commands

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

## Generics

```text
struct Foo<T> {
    data: T,
}

fn identity<T>(x: T) T {
    x
}

pub fn main() void {
    let x = identity::<usize>(42);
    let data = identity(Foo { data: 42 });
    let foo: Foo::<Foo::<usize> > = Foo::<Foo::<usize> > { data };
}
```

## Methods

```text
struct Foo {
    data: u32,

    fn add_one(self: &mut Self) void {
        self.data += 1;
    }
}

interface AddTwo {
    fn add_two(self: &mut Self) void;
}

impl AddTwo for Foo {
    fn add_two(self: &mut Self) void {
        self.data += 2;
    }
}
```

# Tests

## Creating a new integration test

Add an `.oxi` file under `tests/integration/` (or a subdirectory). Tests that should fail use `// @expect-error <code>` annotations. Tests that depend on helpers use `// @auxiliary-module <name>` referencing `tests/integration/<subdirectory?>/auxiliary/<name>.oxi`.

## Test name generation

Test function names are generated at compile time by the `oxic_test` proc-macro (`crates/oxic_test/`). The relative path from `tests/integration/` has its `.oxi` extension stripped, then `/` and `-` are replaced with `_`.

Examples:

- `tests/integration/booleans.oxi` → `booleans`
- `tests/integration/interfaces/interfaces1.oxi` → `interfaces_interfaces1`
- `tests/integration/interfaces/specialization/impl_duplicate_explicit_args.oxi` → `interfaces_specialization_impl_duplicate_explicit_args`

## Running tests

- Run all tests: `just test`
- Run a specific test by generated name: `just test <name>` (e.g. `just test booleans`, `just test interfaces_interfaces1`)
- Run a subset: `just test <partial-name>` uses substring matching (e.g. `just test generics` runs all tests with "generics" in the name)
- Run integration tests only: `cargo test --test integration`
- Run unit tests (inside `src/`): `cargo test --lib <filter>`
- Generate coverage: `just coverage <format>` (uses cargo-tarpaulin)
