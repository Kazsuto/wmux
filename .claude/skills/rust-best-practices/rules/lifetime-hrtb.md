---
title: Use Higher-Ranked Trait Bounds for Lifetime-Generic Callbacks
impact: MEDIUM
impactDescription: enables callbacks that work with references of any lifetime
tags: lifetime, hrtb, for-a, closures, generics
---

## Use Higher-Ranked Trait Bounds for Lifetime-Generic Callbacks

When a function accepts a closure that must work with references of any lifetime, use `for<'a> Fn(&'a T) -> &'a U`. Without it, the lifetime gets locked to a single concrete scope, making the API unnecessarily restrictive.

**Incorrect (lifetime tied to a single concrete scope — rejects valid callbacks):**

```rust
// `'a` is fixed at the call site. The function pointer can only be used
// with references that all share that one concrete lifetime.
fn apply<'a>(items: &'a [String], f: fn(&'a String) -> &'a str) -> Vec<&'a str> {
    items.iter().map(f).collect()
}

// Same problem with a generic bound: 'a is resolved once and fixed.
fn first_chars<'a, F>(items: &'a [String], f: F) -> Vec<&'a str>
where
    F: Fn(&'a String) -> &'a str,
{
    items.iter().map(f).collect()
}
```

**Correct (for<'a> says "this callback must work for ANY lifetime 'a"):**

```rust
// The HRTB `for<'a>` means: F must implement Fn for every possible lifetime.
// This is the standard way to accept lifetime-generic callbacks.
fn apply<F>(items: &[String], f: F) -> Vec<&str>
where
    F: for<'a> Fn(&'a String) -> &'a str,
{
    items.iter().map(f).collect()
}

// HRTB with closures works the same way.
fn transform_all<F>(items: &[String], mut f: F) -> Vec<&str>
where
    F: for<'a> FnMut(&'a String) -> &'a str,
{
    items.iter().map(|s| f(s)).collect()
}

fn usage() {
    let words = vec!["hello world".to_string(), "foo bar".to_string()];

    // Both a function pointer and a closure satisfy the HRTB.
    let first_words = apply(&words, |s| s.split_whitespace().next().unwrap_or(""));
    let as_strs = apply(&words, String::as_str);

    println!("{first_words:?}");
    println!("{as_strs:?}");
}
```

**Note:** In practice, the compiler often infers HRTBs for closures automatically — you may not need to write `for<'a>` explicitly when the types are unambiguous. Write it explicitly when you need to document the constraint clearly or when inference fails. In Rust 2024, `impl Trait` in return position auto-captures all in-scope lifetimes, which reduces some scenarios where HRTBs were previously required.
