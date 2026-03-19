---
title: Never Return References to Local Variables
impact: CRITICAL
impactDescription: prevents dangling references (compile-time error)
tags: lifetime, dangling, return, owned
---

## Never Return References to Local Variables

A function cannot return a reference to data created inside it — that data is dropped when the function returns, leaving the reference dangling. The compiler rejects this, but understanding why prevents the pattern from appearing in the first place.

**Incorrect (returns reference to local — does not compile, data is dropped at end of function):**

```rust
// ERROR: cannot return reference to local variable `greeting`
fn make_greeting(name: &str) -> &str {
    let greeting = format!("Hello, {name}!");
    &greeting  // `greeting` is dropped here; reference would dangle
}

// ERROR: same problem — `result` is local to this function
fn longest_with_suffix(a: &str, b: &str, suffix: &str) -> &str {
    let result = if a.len() >= b.len() {
        format!("{a}{suffix}")
    } else {
        format!("{b}{suffix}")
    };
    &result  // dangling
}
```

**Correct (return owned value, or borrow from input when the data truly comes from outside):**

```rust
use std::borrow::Cow;

// Option 1: return owned String — simple and clear.
fn make_greeting(name: &str) -> String {
    format!("Hello, {name}!")
}

// Option 2: use Cow when the result is sometimes a borrow and sometimes owned.
// Returns borrowed input when no suffix is added, owned when it is.
fn with_optional_suffix<'a>(s: &'a str, suffix: Option<&str>) -> Cow<'a, str> {
    match suffix {
        None => Cow::Borrowed(s),
        Some(sfx) => Cow::Owned(format!("{s}{sfx}")),
    }
}

// Option 3: borrow from input is fine when the reference genuinely comes from there.
fn first_line(text: &str) -> &str {
    // This compiles: we're borrowing from `text`, not from a local.
    text.lines().next().unwrap_or("")
}
```

**Note:** When the borrow checker rejects a returned reference, the fix is almost always to return an owned type. `Cow<'a, str>` (or `Cow<'a, [T]>`) is the idiomatic choice when you need to conditionally avoid allocation — it costs nothing when the borrowed path is taken.
