---
title: Use Cow<str> for Conditional Modification
impact: HIGH
impactDescription: avoids allocation on the fast path
tags: ownership, cow, allocation, performance
---

## Use Cow<str> for Conditional Modification

`Cow` (Clone on Write) returns borrowed data on the fast path and allocates only when mutation is needed.

**Incorrect (always allocates, even when no modification is required):**

```rust
fn normalize_path(path: &str) -> String {
    if path.starts_with("./") {
        path[2..].to_string()
    } else {
        // Fast path still forces an allocation just to return the same bytes
        path.to_string()
    }
}

fn main() {
    // Both calls allocate a new String — even the second one which does no work
    let stripped = normalize_path("./src/main.rs");
    let unchanged = normalize_path("src/main.rs");

    println!("{}", stripped);
    println!("{}", unchanged);
}
```

**Correct (borrows on the fast path, allocates only when stripping is needed):**

```rust
use std::borrow::Cow;

fn normalize_path(path: &str) -> Cow<'_, str> {
    if path.starts_with("./") {
        // Slow path: must strip prefix, so allocate
        Cow::Owned(path[2..].to_string())
    } else {
        // Fast path: return a zero-cost borrow of the original data
        Cow::Borrowed(path)
    }
}

fn main() {
    let stripped = normalize_path("./src/main.rs");   // allocates
    let unchanged = normalize_path("src/main.rs");    // zero allocation

    // Cow<str> derefs to &str transparently
    println!("{}", stripped);
    println!("{}", unchanged);
}
```

**Note:** `Cow` implements `Deref<Target=str>`, so callers use it transparently as `&str`. When the caller needs an owned `String`, they can call `.into_owned()`. This pattern is especially valuable in hot paths such as serializers, path normalizers, and text processors where most inputs require no modification.
