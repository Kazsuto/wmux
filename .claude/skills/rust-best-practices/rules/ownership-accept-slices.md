---
title: Accept Borrowed Slices Over Owned Containers
impact: CRITICAL
impactDescription: eliminates unnecessary allocations and restrictions on callers
tags: ownership, borrowing, slices, str, clippy
---

## Accept Borrowed Slices Over Owned Containers

Functions should accept the most general borrowed form. Use `&str` not `&String`, `&[T]` not `&Vec<T>`, `&T` not `&Box<T>`.

**Incorrect (forces callers to own a String):**

```rust
fn search(haystack: &String, needle: &String) -> bool {
    haystack.contains(needle.as_str())
}

fn main() {
    let haystack = String::from("the quick brown fox");
    let needle = String::from("quick");
    // Callers must own a String — cannot pass a &str literal directly
    println!("{}", search(&haystack, &needle));
}
```

**Correct (accepts both &str and String via deref coercion):**

```rust
fn search(haystack: &str, needle: &str) -> bool {
    haystack.contains(needle)
}

fn main() {
    let owned = String::from("the quick brown fox");

    // Works with a String reference (deref coercion)
    println!("{}", search(&owned, "quick"));

    // Also works with plain string literals — no allocation needed
    println!("{}", search("the quick brown fox", "quick"));
}
```

**Note:** Enforced by Clippy lint `clippy::ptr_arg`. The same principle applies to `&[T]` instead of `&Vec<T>`, and `&T` instead of `&Box<T>` — always prefer the unsized or slice form so callers are not forced into a specific owning container.
