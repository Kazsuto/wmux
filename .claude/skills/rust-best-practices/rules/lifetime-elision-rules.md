---
title: Know the Three Lifetime Elision Rules
impact: HIGH
impactDescription: avoids noisy redundant lifetime annotations
tags: lifetime, elision, readability, idioms
---

## Know the Three Lifetime Elision Rules

Rust has three elision rules that let you omit annotations in common cases. Adding explicit lifetimes where elision applies is noise that obscures intent.

**Incorrect (redundant explicit lifetimes — elision covers all of these):**

```rust
// Rule 2 applies: single input reference -> output gets same lifetime.
fn first_word<'a>(s: &'a str) -> &'a str {
    s.split_whitespace().next().unwrap_or("")
}

// Rule 3 applies: &self method -> output gets 'self lifetime.
struct Config {
    name: String,
}
impl Config {
    fn name<'a>(&'a self) -> &'a str {
        &self.name
    }
}
```

**Correct (let elision do its job — identical semantics, far less noise):**

```rust
// Rule 2: single &str input -> output lifetime is the same. No annotation needed.
fn first_word(s: &str) -> &str {
    s.split_whitespace().next().unwrap_or("")
}

// Rule 3: &self method output defaults to 'self. No annotation needed.
struct Config {
    name: String,
}
impl Config {
    fn name(&self) -> &str {
        &self.name
    }
}

// IMPORTANT: elision does NOT cover multiple input references with an output reference.
// Here explicit lifetimes ARE required because there are two input refs.
fn strip_one<'a>(s: &'a str, prefix: &str) -> &'a str {
    s.strip_prefix(prefix).unwrap_or(s)
}
```

**Note:** The three rules are: (1) each reference parameter gets its own distinct lifetime; (2) if there is exactly one input lifetime, it is assigned to all output lifetimes; (3) if one of the inputs is `&self` or `&mut self`, its lifetime is assigned to all output lifetimes. When the compiler cannot resolve elision — typically with multiple input references and an output reference — you must annotate explicitly to declare intent.
