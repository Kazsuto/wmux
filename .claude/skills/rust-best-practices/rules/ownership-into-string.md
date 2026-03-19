---
title: Accept impl Into<String> When Ownership Is Needed
impact: HIGH
impactDescription: flexible API that avoids redundant .to_string() at call sites
tags: ownership, into, api-design, ergonomics
---

## Accept impl Into<String> When Ownership Is Needed

When a function must own the data, accepting `impl Into<String>` lets callers pass `String` (zero-cost move) or `&str` (one allocation).

**Incorrect (forces callers to always convert with .to_string()):**

```rust
struct Config {
    name: String,
}

impl Config {
    fn new(name: String) -> Self {
        Config { name }
    }
}

fn main() {
    // Must pay .to_string() even when passing a literal
    let cfg1 = Config::new("default".to_string());

    let existing = String::from("custom");
    // Move works, but the API forced us to know this distinction
    let cfg2 = Config::new(existing);

    println!("{} / {}", cfg1.name, cfg2.name);
}
```

**Correct (accepts both &str and String — callers pay exactly one allocation):**

```rust
struct Config {
    name: String,
}

impl Config {
    fn new(name: impl Into<String>) -> Self {
        Config { name: name.into() }
    }
}

fn main() {
    // &str literal: one allocation inside new(), no boilerplate at call site
    let cfg1 = Config::new("default");

    let existing = String::from("custom");
    // String: zero-cost move, no extra allocation
    let cfg2 = Config::new(existing);

    println!("{} / {}", cfg1.name, cfg2.name);
}
```

**Note:** For hot paths where monomorphization cost matters, prefer a concrete `&str` parameter with an explicit `.to_string()` inside, since `impl Into<String>` generates two copies of the function (one for `&str`, one for `String`). For most APIs the ergonomic benefit outweighs the marginal compile-time cost.
