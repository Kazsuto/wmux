---
title: Avoid Needless collect — Keep Iterators Lazy
impact: MEDIUM
impactDescription: eliminates unnecessary intermediate allocations
tags: iterator, collect, lazy, allocation, clippy
---

## Avoid Needless collect — Keep Iterators Lazy

Clippy's needless_collect lint catches cases where collect::<Vec<_>>() is followed by an operation that could be done on the iterator directly.

**Incorrect (collects into Vec just to iterate or extend again):**

```rust
// Collects into Vec just to iterate again
let names: Vec<String> = users.iter()
    .map(|u| u.name.clone())
    .collect();
for name in names.iter() {
    println!("{name}");
}

// Collects before passing to a function taking IntoIterator
let ids: Vec<u64> = input.iter().map(|x| x.id).collect();
target.extend(ids);
```

**Correct (operate on the iterator directly):**

```rust
// Iterate directly
for name in users.iter().map(|u| &u.name) {
    println!("{name}");
}

// Pass iterator directly to extend
target.extend(input.iter().map(|x| x.id));
```

**Note:** Legitimate reasons to collect: reuse data multiple times, need random access, need length before processing.
