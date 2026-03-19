---
title: Reuse Allocations in Hot Loops
impact: HIGH
impactDescription: eliminates repeated allocation/deallocation in tight loops
tags: memory, allocation, loops, performance, clear
---

## Reuse Allocations in Hot Loops

Move `String`/`Vec`/`HashMap` creation outside loops and use `.clear()` to reset while keeping the underlying capacity.

**Incorrect (allocation every iteration):**

```rust
fn process_lines(lines: &[&str]) -> Vec<String> {
    let mut results = Vec::new();

    for line in lines {
        // Allocates a fresh String on every iteration — N allocations total
        let mut temp = String::new();
        temp.push_str(line.trim());
        temp.push_str(" [processed]");
        results.push(temp);
    }

    results
}

fn main() {
    let lines = vec!["  hello  ", "  world  ", "  rust  "];
    let processed = process_lines(&lines);
    for p in &processed {
        println!("{}", p);
    }
}
```

**Correct (one allocation reused across all iterations):**

```rust
fn process_lines(lines: &[&str]) -> Vec<String> {
    let mut results = Vec::with_capacity(lines.len());
    // Allocate once outside the loop
    let mut temp = String::with_capacity(64);

    for line in lines {
        // Clear resets length to 0 but keeps heap capacity — zero allocation
        temp.clear();
        temp.push_str(line.trim());
        temp.push_str(" [processed]");
        // Clone only when we need to store the value; temp is reused next iteration
        results.push(temp.clone());
    }

    results
}

fn main() {
    let lines = vec!["  hello  ", "  world  ", "  rust  "];
    let processed = process_lines(&lines);
    for p in &processed {
        println!("{}", p);
    }
}
```

**Note:** Pre-allocate with `String::with_capacity()` or `Vec::with_capacity()` if the approximate size is known — this avoids even the first reallocation. The same pattern applies to `HashMap`: create it once before the loop and call `.clear()` each iteration to retain its bucket array.
