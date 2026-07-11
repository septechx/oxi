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

## Generics

```text
struct Foo<T> {
    data: T,
}

fn identity<T>(x: T) T {
    x
}

pub fn main() {
    let x = identity::<usize>(42);
    let data = identity(Foo { data: 42 });
    let foo: Foo::<Foo::<usize> > = Foo::<Foo::<usize> > { data };
}
```

The tests (`tests/integration/**/*.oxi`) contain more example code
