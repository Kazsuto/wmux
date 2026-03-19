---
title: Use SmallVec for Small, Short-Lived Collections
impact: MEDIUM
impactDescription: eliminates heap allocation for common small-collection cases
tags: memory, smallvec, stack, allocation, optimization
---

## Use SmallVec for Small, Short-Lived Collections

`SmallVec` stores up to N elements inline on the stack, spilling to the heap only when that limit is exceeded. Ideal for collections that are usually small.

**Incorrect (always heap-allocates even for a fixed 4-element result):**

```rust
fn neighbors(x: i32, y: i32) -> Vec<(i32, i32)> {
    // Vec always allocates on the heap — even for these 4 constant-size results
    vec![
        (x - 1, y),
        (x + 1, y),
        (x, y - 1),
        (x, y + 1),
    ]
}

fn main() {
    let adj = neighbors(5, 5);
    for (nx, ny) in &adj {
        println!("({}, {})", nx, ny);
    }
}
```

**Correct (inline up to 4 elements — zero heap allocations in the common case):**

```rust
use smallvec::{SmallVec, smallvec};

fn neighbors(x: i32, y: i32) -> SmallVec<[(i32, i32); 4]> {
    // Up to 4 pairs live entirely on the stack; spills to heap only if more are added
    smallvec![
        (x - 1, y),
        (x + 1, y),
        (x, y - 1),
        (x, y + 1),
    ]
}

fn main() {
    let adj = neighbors(5, 5);
    for (nx, ny) in &adj {
        println!("({}, {})", nx, ny);
    }
}
```

**Note:** If the size is always fixed and known at compile time, return a plain array instead — it is even cheaper because there is no length/capacity bookkeeping. Use `SmallVec` when the size varies but is usually small, such as argument lists, token spans, or graph adjacency lists. Add `smallvec` to `Cargo.toml` with `smallvec = "1"`. For a `#![forbid(unsafe_code)]` codebase, consider `tinyvec` as an alternative that uses no unsafe code internally. For fixed-maximum-size collections, `arrayvec` avoids the heap-spill codepath entirely.
