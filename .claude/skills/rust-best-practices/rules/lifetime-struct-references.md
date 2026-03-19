---
title: Structs Holding References Need Explicit Lifetimes
impact: CRITICAL
impactDescription: required by the compiler; no elision for struct fields
tags: lifetime, struct, references, annotation
---

## Structs Holding References Need Explicit Lifetimes

Unlike functions, lifetime elision does NOT apply to struct field definitions. Any struct storing a reference must declare a lifetime parameter that ties the struct's validity to the borrowed data.

**Incorrect (missing lifetime specifier — this does not compile):**

```rust
// ERROR: missing lifetime specifier
// The compiler has no way to know how long `text` must stay valid.
struct Excerpt {
    text: &str,
}

// ERROR: same problem with multiple borrowed fields.
struct ParseContext {
    source: &str,
    current_token: &str,
}
```

**Correct (explicit lifetime parameter ties the struct to its borrowed data):**

```rust
// The struct cannot outlive the data that `text` points into.
struct Excerpt<'a> {
    text: &'a str,
}

impl<'a> Excerpt<'a> {
    fn new(text: &'a str) -> Self {
        Self { text }
    }

    fn display(&self) {
        println!("{}", self.text);
    }
}

// Multiple fields can share a lifetime (they all must live at least that long)
// or have separate ones if they can come from independent sources.
struct ParseContext<'src> {
    source: &'src str,
    current_token: &'src str,
}

fn usage(document: &str) {
    let excerpt = Excerpt::new(&document[0..10]);
    excerpt.display();
    // `excerpt` cannot outlive `document` — the borrow checker enforces this.
}
```

**Note:** Consider owning the data (`String` instead of `&str`, `Vec<T>` instead of `&[T]`) when borrowing is not a deliberate performance choice. Owned types simplify struct lifetimes significantly and are often fast enough. Reserve borrowed structs for hot paths where eliminating allocations is proven necessary.
