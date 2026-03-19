---
title: Use expect() Over unwrap() with Invariant Messages
impact: HIGH
impactDescription: provides actionable panic messages explaining the invariant
tags: error, expect, unwrap, panic, invariants
---

## Use expect() Over unwrap() with Invariant Messages

When a value must exist due to program invariants, use `expect()` with a message explaining WHY the value should be present — not what went wrong.

**Incorrect (unwrap() with zero context — panic message is useless for debugging):**

```rust
fn process_input(input: &str) -> u32 {
    // Panics with: "called `Option::unwrap()` on a `None` value"
    // or: "called `Result::unwrap()` on an `Err` value: ParseIntError { ... }"
    // Neither tells you what invariant was violated.
    input.trim().parse::<u32>().unwrap()
}

fn get_first(items: &[String]) -> &str {
    // Panics with: "called `Option::unwrap()` on a `None` value"
    // No indication of which collection or why it was expected to be non-empty.
    items.first().unwrap()
}
```

**Correct (expect() messages state the invariant — immediately actionable in a backtrace):**

```rust
fn process_input(input: &str) -> u32 {
    // Clear: the caller is responsible for validating before calling this.
    input
        .trim()
        .parse::<u32>()
        .expect("input was validated as a positive integer by validate_input()")
}

fn get_first(items: &[String]) -> &str {
    // Clear: whoever calls this must guarantee the slice is non-empty.
    items
        .first()
        .expect("items must be non-empty; caller must check before calling get_first()")
}

// Startup: panicking on misconfiguration is intentional.
fn init() {
    let port: u16 = std::env::var("PORT")
        .expect("PORT environment variable must be set")
        .parse()
        .expect("PORT must be a valid u16 port number");
}
```

**Note:** Reserve `expect`/`unwrap` for: startup/initialization code, tests, and operations that are provably infallible (e.g., `"42".parse::<u32>().expect("literal is valid")`). Prefer returning `Result` in request handlers, library functions, and any code path that runs repeatedly at runtime.
