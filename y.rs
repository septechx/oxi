struct Foo {}

trait WBar {
    type Bar<T>;
}

impl WBar for Foo {
    type Bar<T> = Vec<T>;
}
