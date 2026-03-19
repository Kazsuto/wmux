---
title: Doc Tests to Keep Examples Honest
impact: MEDIUM
impactDescription: prevents documentation rot by compiling and running doc examples
tags: testing, doc-tests, documentation, examples, cargo-test
---

## Doc Tests to Keep Examples Honest

Place executable code examples in /// doc comments. cargo test runs them automatically, ensuring docs never drift from reality.

**Incorrect (plain text examples — never compiled, silently become wrong):**

```rust
// add — Adds two numbers.
//
// Example:
//   add(2, 3) == 5   <- never verified, can silently lie
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
```

**Correct (fenced code blocks in /// comments — compiled and run by cargo test):**

```rust
/// Adds two numbers together.
///
/// # Examples
///
/// ```
/// use my_crate::add;
///
/// assert_eq!(add(2, 3), 5);
/// assert_eq!(add(-1, 1), 0);
/// ```
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

/// Divides two numbers, returning None if the divisor is zero.
///
/// # Examples
///
/// ```
/// use my_crate::checked_div;
///
/// assert_eq!(checked_div(10, 2), Some(5));
/// assert_eq!(checked_div(10, 0), None);
/// ```
///
/// Does not compile with wrong argument types:
///
/// ```compile_fail
/// my_crate::checked_div("ten", "two");
/// ```
pub fn checked_div(a: i32, b: i32) -> Option<i32> {
    if b == 0 { None } else { Some(a / b) }
}
```

**Note:** Use # prefix to hide boilerplate lines. Use no_run for examples requiring external resources, should_panic for panic examples, compile_fail for negative compile examples. In the Rust 2024 edition, doc tests are compiled into a single binary instead of separate executables, dramatically reducing test compilation time for crates with many doc tests.
