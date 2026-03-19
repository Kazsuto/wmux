---
title: Organize Tests in #[cfg(test)] Modules and tests/ Directory
impact: HIGH
impactDescription: ensures test code is stripped from releases and follows idiomatic layout
tags: testing, organization, cfg-test, integration, unit
---

## Organize Tests in #[cfg(test)] Modules and tests/ Directory

Place unit tests in #[cfg(test)] mod tests at the bottom of each source file. Place integration tests in the top-level tests/ directory.

**Incorrect (test function at module level — always compiled):**

```rust
pub fn add(a: i32, b: i32) -> i32 { a + b }

// Test at module level -- always compiled!
pub fn test_add() {
    assert_eq!(add(2, 3), 5);
}
```

**Correct (unit tests gated behind #[cfg(test)], integration tests in tests/):**

```rust
// src/math.rs
pub fn add(a: i32, b: i32) -> i32 { a + b }

fn saturating_add(a: i32, b: i32) -> i32 {
    a.checked_add(b).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_basic() {
        assert_eq!(add(2, 3), 5);
    }

    #[test]
    fn test_saturating_add_overflow() {
        assert_eq!(saturating_add(i32::MAX, 1), i32::MAX);
    }
}
```

```rust
// tests/integration_test.rs
use my_crate::add;

#[test]
fn test_add_via_public_api() {
    assert_eq!(add(10, 20), 30);
}
```

**Note:** Doc tests in /// comments serve as both documentation and tests. In the Rust 2024 edition, doc tests are compiled into a single binary rather than each being a separate executable, yielding massive speedups. Consider `cargo-nextest` as an alternative test runner offering process-per-test isolation, better parallelism, retries, and JUnit output for CI/CD.
