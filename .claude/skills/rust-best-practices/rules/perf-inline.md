---
title: Use #[inline] Strategically for Cross-Crate Functions
impact: HIGH
impactDescription: enables cross-crate inlining for small hot functions
tags: performance, inline, optimization, cross-crate, lto
---

## Use #[inline] Strategically for Cross-Crate Functions

Without `#[inline]`, the compiler cannot see function bodies across crate boundaries. Apply to small, hot, public library functions.

**Incorrect (callers from other crates cannot inline):**

```rust
// mylib/src/lib.rs
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn multiply(a: i32, b: i32) -> i32 {
    a * b
}

// The compiler only sees the symbol, not the body — no inlining possible
// for crates that depend on mylib.
```

**Correct (body serialized into crate metadata):**

```rust
// mylib/src/lib.rs
#[inline]
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[inline]
pub fn multiply(a: i32, b: i32) -> i32 {
    a * b
}

// For large, rarely-called functions (e.g., error formatting), keep them out:
#[cold]
#[inline(never)]
pub fn handle_error(code: i32) -> String {
    format!("fatal error: code {code}")
}
```

**Note:** Use `#[cold]` `#[inline(never)]` for error paths to keep them out of the instruction cache. Do not apply `#[inline]` indiscriminately — it increases compile times and binary size. For application binaries, prefer `lto = true` in `Cargo.toml` instead, which gives the linker full visibility across crates without manual annotation.
