---
title: Choose iter() vs into_iter() vs iter_mut() by Ownership Need
impact: MEDIUM
impactDescription: prevents unnecessary consumption or cloning of collections
tags: iterator, ownership, iter, into-iter, iter-mut
---

## Choose iter() vs into_iter() vs iter_mut() by Ownership Need

iter() yields &T (collection still usable), iter_mut() yields &mut T (collection still usable), into_iter() yields T (collection consumed).

**Incorrect (consuming or cloning unnecessarily):**

```rust
let names = vec!["Alice".to_string(), "Bob".to_string()];
// Consumes names — cannot use afterward
for name in names.into_iter() {
    println!("{name}");
}
// println!("{:?}", names);  // ERROR: value moved
```

```rust
// --- Separate example ---
// Or: cloning unnecessarily
let names = vec!["Alice".to_string(), "Bob".to_string()];
let lengths: Vec<usize> = names.iter()
    .map(|n| n.clone().len())  // Unnecessary clone
    .collect();
```

**Correct (choose the iterator form that matches the ownership need):**

```rust
let mut names = vec!["Alice".to_string(), "Bob".to_string()];

// Borrow: iter() when you need the collection afterward
let lengths: Vec<usize> = names.iter().map(|n| n.len()).collect();
println!("Names still available: {:?}", names);

// Mutate in place: iter_mut()
for name in names.iter_mut() {
    *name = name.to_uppercase();
}

// Consume: into_iter() when done with the collection
let owned: Vec<String> = names.into_iter()
    .filter(|n| n.starts_with('A'))
    .collect();
```

**Note:** In for loops, `for x in &col` = iter(), `for x in &mut col` = iter_mut(), `for x in col` = into_iter().
