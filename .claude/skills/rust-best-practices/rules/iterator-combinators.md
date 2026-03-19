---
title: Prefer Option/Result Combinators Over Nested match
impact: MEDIUM
impactDescription: reduces nesting and improves readability
tags: option, result, combinators, functional
---

## Prefer Option/Result Combinators Over Nested match

Use map, and_then, unwrap_or_else, ok_or, and filter to compose transformations functionally. Reserve match for cases needing different logic per arm.

**Incorrect (deeply nested match arms):**

```rust
fn get_user_display_name(id: u64) -> String {
    match find_user(id) {
        Some(user) => {
            match user.display_name {
                Some(name) => {
                    if name.is_empty() {
                        "Anonymous".to_string()
                    } else {
                        name
                    }
                }
                None => "Anonymous".to_string(),
            }
        }
        None => "Unknown User".to_string(),
    }
}
```

**Correct (combinator chain — flat and readable):**

```rust
fn get_user_display_name(id: u64) -> String {
    find_user(id)
        .and_then(|user| user.display_name)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Anonymous".to_string())
}

// Converting Option to Result:
fn get_user_email(id: u64) -> Result<String, UserError> {
    find_user(id)
        .ok_or(UserError::NotFound(id))?
        .email
        .ok_or(UserError::NoEmail(id))
}
```

**Note:** Key combinators: map (transform inner), and_then (chain Options/Results), unwrap_or_else (lazy fallback), ok_or (Option→Result), map_err (transform error). Since Rust 1.88, let chains offer another alternative for sequential Option/Result checks (requires **edition 2024** — not available on earlier editions even with Rust 1.88+):

```rust
if let Some(user) = find_user(id)
    && let Some(name) = user.display_name
    && !name.is_empty()
{
    name
} else {
    "Anonymous".to_string()
}
```
