---
title: Prefer Stack Allocation; Box Only When Necessary
impact: CRITICAL
impactDescription: stack allocation is effectively free vs heap allocator overhead
tags: memory, stack, heap, box, allocation
---

## Prefer Stack Allocation; Box Only When Necessary

Stack allocation is a pointer bump. Use `Box` only for recursive types, large data, trait objects, or transferring ownership across scopes where the size must be erased.

**Incorrect (pointless heap allocation for a small, fixed-size value):**

```rust
fn compute() -> Box<[f64; 3]> {
    // Allocates 24 bytes on the heap for no benefit
    Box::new([1.0, 2.0, 3.0])
}

fn main() {
    let result = compute();
    // Caller must dereference through a heap pointer for a value that fits in registers
    println!("{:?}", *result);
}
```

**Correct (returned on the stack or directly via registers):**

```rust
fn compute() -> [f64; 3] {
    // No heap involvement — compiler returns this in registers or via stack slot
    [1.0, 2.0, 3.0]
}

fn main() {
    let result = compute();
    println!("{:?}", result);
}
```

**Note:** LLVM can sometimes optimize away trivial `Box` allocations through inlining and SROA (Scalar Replacement of Aggregates), but this is unreliable — do not depend on it. Prefer keeping small fixed-size values on the stack explicitly. For very large arrays (>100KB), `Box` is appropriate to avoid stack overflow. Default stack sizes are OS-dependent: ~8MB on Linux/macOS but **~1MB on Windows** for the main thread; ~2MB for spawned Rust threads (set by `std::thread::Builder`).
