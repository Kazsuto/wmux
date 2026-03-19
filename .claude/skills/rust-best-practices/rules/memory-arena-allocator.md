---
title: Use Arena Allocators for Same-Lifetime Objects
impact: MEDIUM
impactDescription: pointer-bump allocation + single bulk deallocation
tags: memory, arena, bumpalo, allocation, performance
---

## Use Arena Allocators for Same-Lifetime Objects

For thousands of small objects sharing a common lifetime (AST nodes, per-request allocations), arena allocators like `bumpalo` allocate from a contiguous slab and free everything at once.

**Incorrect (thousands of separate heap allocations and frees):**

```rust
#[derive(Debug)]
enum Expr {
    Num(f64),
    Add(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
}

fn build_tree(depth: u32) -> Box<Expr> {
    if depth == 0 {
        // Each leaf: one Box allocation
        return Box::new(Expr::Num(1.0));
    }
    // Each interior node: one Box allocation + two recursive allocations
    Box::new(Expr::Add(
        build_tree(depth - 1),
        build_tree(depth - 1),
    ))
}

fn main() {
    // depth=12 → 2^13 - 1 = 8191 separate heap allocations + 8191 separate frees
    let tree = build_tree(12);
    println!("{:?}", std::mem::discriminant(tree.as_ref()));
}
```

**Correct (pointer-bump allocation — single deallocation when the arena drops):**

```rust
use bumpalo::Bump;

#[derive(Debug)]
enum Expr<'arena> {
    Num(f64),
    Add(&'arena Expr<'arena>, &'arena Expr<'arena>),
    Mul(&'arena Expr<'arena>, &'arena Expr<'arena>),
}

fn build_tree<'arena>(bump: &'arena Bump, depth: u32) -> &'arena Expr<'arena> {
    if depth == 0 {
        // Pointer bump — no allocator overhead, no individual free needed
        return bump.alloc(Expr::Num(1.0));
    }
    bump.alloc(Expr::Add(
        build_tree(bump, depth - 1),
        build_tree(bump, depth - 1),
    ))
}

fn main() {
    // One contiguous slab created here
    let bump = Bump::new();

    // depth=12 → 8191 pointer-bump allocations, zero individual frees
    let tree = build_tree(&bump, 12);
    println!("{:?}", std::mem::discriminant(tree));

    // All 8191 nodes freed here in a single operation when `bump` drops
}
```

**Note:** Used by rustc internally. Ideal for compilers, parsers, and per-request server allocations. For single-type arenas without lifetime annotations, consider `typed-arena`. Note that neither `typed-arena` nor `bumpalo` is thread-safe (`Bump` is `!Sync` regardless of feature flags); the `allocator_api` feature only enables the unstable `Allocator` trait for use with standard collections (`Vec<T, &Bump>`), it does **not** add thread safety. For concurrent arena allocation, consider `bumpalo-herd` (a pool of per-thread `Bump` allocators) or `bump-scope`.
