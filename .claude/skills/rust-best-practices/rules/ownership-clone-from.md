---
title: Use clone_from to Reuse Existing Allocations
impact: MEDIUM
impactDescription: avoids free+malloc cycle when overwriting owned values
tags: ownership, clone, allocation, optimization
---

## Use clone_from to Reuse Existing Allocations

When assigning a clone to an existing variable, `clone_from` can reuse the existing heap allocation instead of dropping and re-allocating.

**Incorrect (drops the old allocation then allocates a fresh one):**

```rust
fn rotate_buffer(buffer: &mut String, source: &String) {
    // Drops the existing heap buffer, then allocates new memory to hold the clone
    *buffer = source.clone();
}

fn main() {
    let source = String::from("hello, world — this is a moderately long string");
    let mut buffer = String::with_capacity(64);

    for _ in 0..1000 {
        rotate_buffer(&mut buffer, &source);
        // Each iteration: free old allocation + malloc new allocation
    }

    println!("{}", buffer);
}
```

**Correct (reuses the existing allocation when it has sufficient capacity):**

```rust
fn rotate_buffer(buffer: &mut String, source: &String) {
    // Reuses the existing heap buffer if it is large enough — no free+malloc
    buffer.clone_from(source);
}

fn main() {
    let source = String::from("hello, world — this is a moderately long string");
    let mut buffer = String::with_capacity(64);

    for _ in 0..1000 {
        rotate_buffer(&mut buffer, &source);
        // After the first iteration the allocation is stable — zero extra allocations
    }

    println!("{}", buffer);
}
```

**Note:** Clippy lint `assigning_clones` (introduced in late 2023, moved to the `pedantic` group in mid-2024 — allow by default) suggests this transformation automatically when enabled. Enable with `#[warn(clippy::assigning_clones)]` or by running `clippy --warn clippy::pedantic`.
