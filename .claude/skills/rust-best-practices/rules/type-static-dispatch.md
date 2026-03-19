---
title: Prefer impl Trait (Static Dispatch) by Default
impact: HIGH
impactDescription: enables monomorphization, inlining, and zero-cost abstractions
tags: type, impl-trait, dyn-trait, dispatch, generics
---

## Prefer impl Trait (Static Dispatch) by Default

Static dispatch (`impl Trait` / generics) enables monomorphization and inlining. Use `dyn Trait` only for heterogeneous collections or plugin architectures.

**Incorrect (unnecessary vtable lookup via dynamic dispatch):**

```rust
trait Handler {
    fn handle(&self, input: &str) -> String;
}

struct EchoHandler;

impl Handler for EchoHandler {
    fn handle(&self, input: &str) -> String {
        format!("Echo: {}", input)
    }
}

// Forces a vtable lookup on every call — no inlining possible
fn process(handler: &dyn Handler, input: &str) -> String {
    handler.handle(input)
}

fn main() {
    let h = EchoHandler;
    println!("{}", process(&h, "hello"));
}
```

**Correct (monomorphized, inlineable, zero-cost abstraction):**

```rust
trait Handler {
    fn handle(&self, input: &str) -> String;
}

struct EchoHandler;
struct UpperHandler;

impl Handler for EchoHandler {
    fn handle(&self, input: &str) -> String {
        format!("Echo: {}", input)
    }
}

impl Handler for UpperHandler {
    fn handle(&self, input: &str) -> String {
        input.to_uppercase()
    }
}

// Compiler generates a specialized version for each concrete type
fn process(handler: &impl Handler, input: &str) -> String {
    handler.handle(input)
}

// Use dyn Trait only when you genuinely need heterogeneous dispatch
fn process_all(handlers: &[Box<dyn Handler>], input: &str) {
    for h in handlers {
        println!("{}", h.handle(input));
    }
}

fn main() {
    let echo = EchoHandler;
    let upper = UpperHandler;

    println!("{}", process(&echo, "hello"));
    println!("{}", process(&upper, "hello"));

    // Heterogeneous collection — dyn Trait is appropriate here
    let handlers: Vec<Box<dyn Handler>> = vec![
        Box::new(EchoHandler),
        Box::new(UpperHandler),
    ];
    process_all(&handlers, "world");
}
```

**Note:** For closed sets of types, consider an `enum` with `match` instead of `dyn Trait` — it retains static dispatch and allows exhaustiveness checking. Since Rust 1.75, `impl Trait` can be used in trait return positions (RPITIT): `fn method(&self) -> impl Display`. Since Rust 1.82, use `+ use<'a, T>` to control precise capturing in return-position `impl Trait`. Since Rust 1.86, trait upcasting is stable: `&dyn SubTrait` can be coerced to `&dyn SuperTrait` directly, making `dyn Trait` hierarchies more practical. The term "object safety" has been renamed to "dyn compatibility" in the 2024 edition.
