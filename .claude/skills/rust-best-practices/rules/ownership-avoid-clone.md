---
title: Avoid Unnecessary .clone() — Borrow Instead
impact: CRITICAL
impactDescription: eliminates deep copies and heap allocations
tags: ownership, clone, borrowing, performance
---

## Avoid Unnecessary .clone() — Borrow Instead

Every `.clone()` performs a deep copy. Replace with borrowing when the function only needs to read data.

**Incorrect (clones the entire Vec and each String inside it):**

```rust
fn print_report(data: Vec<String>) {
    for item in &data {
        println!("{}", item);
    }
}

fn main() {
    let records = vec![
        String::from("alpha"),
        String::from("beta"),
        String::from("gamma"),
    ];

    // Clones the entire Vec<String> plus every String inside — O(n) allocations
    print_report(records.clone());

    // records is still usable here, but we paid the full clone cost
    println!("record count: {}", records.len());
}
```

**Correct (zero-cost borrow — no allocation):**

```rust
fn print_report(data: &[String]) {
    for item in data {
        println!("{}", item);
    }
}

fn main() {
    let records = vec![
        String::from("alpha"),
        String::from("beta"),
        String::from("gamma"),
    ];

    // Passes a borrowed slice — no allocation, no copy
    print_report(&records);

    // records is still fully owned and usable
    println!("record count: {}", records.len());
}
```

**Note:** Clippy lint `redundant_clone` catches `.clone()` calls where the original value is never used afterward (meaning a move would have sufficed). For the pattern shown here — where a function takes ownership but only reads — there is no automatic lint; you must manually refactor the function signature to accept a reference. See also the `ownership-accept-slices` rule.
