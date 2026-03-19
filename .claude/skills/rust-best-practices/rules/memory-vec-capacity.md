---
title: Pre-allocate Vec with with_capacity
impact: HIGH
impactDescription: eliminates ~17 reallocations for 100k elements
tags: memory, vec, allocation, capacity, performance
---

## Pre-allocate Vec with with_capacity

Use `Vec::with_capacity(n)` when size is known or estimable. Eliminates repeated reallocations during growth.

**Incorrect (starts at capacity 0, triggers ~17 reallocations for 100k items):**

```rust
fn collect_squares(n: u64) -> Vec<u64> {
    // Capacity 0 — Vec doubles on each overflow: 0→1→2→4→8→...→131072
    // That is ~17 reallocations and memcpy calls for n=100_000
    let mut results = Vec::new();

    for i in 0..n {
        results.push(i * i);
    }

    results
}

fn main() {
    let squares = collect_squares(100_000);
    println!("last square: {}", squares.last().unwrap());
}
```

**Correct (one allocation, exact size, zero reallocations):**

```rust
fn collect_squares(n: u64) -> Vec<u64> {
    // Allocates space for exactly n elements up front
    let mut results = Vec::with_capacity(n as usize);

    for i in 0..n {
        results.push(i * i);
    }

    results
}

fn main() {
    let squares = collect_squares(100_000);
    println!("last square: {}", squares.last().unwrap());
}
```

**Note:** Use `vec.reserve(additional)` before bulk operations when the final size is not known at construction time but becomes known mid-loop. Use `vec.shrink_to_fit()` on long-lived vectors after their peak size to return excess memory to the OS. The standard `Iterator::collect()` already calls `with_capacity` internally when the iterator implements `ExactSizeIterator`, so prefer `.collect()` over a manual push loop when using iterator chains.
