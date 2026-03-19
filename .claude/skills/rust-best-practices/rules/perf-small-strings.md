---
title: Use Small String Optimization for Short, Frequent Strings
impact: MEDIUM
impactDescription: eliminates heap allocation for strings under 24 bytes
tags: performance, string, sso, compact-str, allocation
---

## Use Small String Optimization for Short, Frequent Strings

Standard `String` always heap-allocates. For workloads dominated by short strings (under 24 bytes), use `compact_str` or `smol_str` to store them inline on the stack.

**Incorrect (heap allocation per token):**

```rust
// Each call to .to_string() allocates on the heap:
// pointer (8 bytes) + length (8 bytes) + capacity (8 bytes) = 24 bytes overhead
// plus a separate heap buffer for the string data itself.
fn tokenize(input: &str) -> Vec<String> {
    input
        .split_whitespace()
        .map(|s| s.to_string())
        .collect::<Vec<String>>()
}

fn main() {
    // Tokenizing millions of short words (e.g., "the", "a", "is")
    // causes millions of tiny heap allocations.
    let tokens = tokenize("the quick brown fox jumps over the lazy dog");
    println!("{} tokens", tokens.len());
}
```

**Correct (inline storage for short strings — no heap allocation):**

```rust
use compact_str::CompactString;

// CompactString stores strings up to 24 bytes inline (on 64-bit platforms),
// falling back to a heap allocation only for longer strings.
// The struct itself is the same size as String (24 bytes).
fn tokenize(input: &str) -> Vec<CompactString> {
    input
        .split_whitespace()
        .map(CompactString::from)
        .collect::<Vec<CompactString>>()
}

fn main() {
    let tokens = tokenize("the quick brown fox jumps over the lazy dog");
    println!("{} tokens", tokens.len());

    // CompactString is a drop-in replacement — same API as String.
    for token in &tokens {
        println!("{token}");
    }
}
```

```toml
# Cargo.toml
[dependencies]
compact_str = "0.9"
```

**Note:** Choose the right crate for your use case. `compact_str` is best when you need a mutable `String` replacement with SSO. `smol_str` and `ecow` are best when strings are mostly immutable and cloning must be O(1) (they use reference counting for long strings). Always profile before switching — the benefit is most pronounced when allocator pressure is the measured bottleneck.
