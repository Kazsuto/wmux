---
title: Use MaybeUninit Instead of mem::uninitialized
impact: CRITICAL
impactDescription: prevents instant undefined behavior for types with validity invariants
tags: unsafe, maybeuninit, uninitialized, memory, ub
---

## Use MaybeUninit Instead of mem::uninitialized

`std::mem::uninitialized` is deprecated and causes instant undefined behavior for most types. Use `MaybeUninit<T>` to explicitly manage initialization state.

**Incorrect (instant UB — drop frees garbage pointer):**

```rust
fn bad_uninitialized_string() -> String {
    // `String` contains a pointer, a length, and a capacity.
    // "Uninitialized" means those fields contain arbitrary bytes from the stack.
    // The compiler is allowed to assume a `String` is always valid —
    // creating one in this state is immediate undefined behavior.
    // When this function returns and the value is dropped, `drop` will call
    // `dealloc` on a garbage pointer, corrupting the heap or crashing.
    #[allow(deprecated)]
    let value: String = unsafe { std::mem::uninitialized() };
    value
}

fn bad_uninitialized_bool() -> bool {
    // `bool` has a validity invariant: only 0 (false) and 1 (true) are valid.
    // Any other bit pattern is instant UB — the compiler may miscompile
    // branches that read this value.
    #[allow(deprecated)]
    let flag: bool = unsafe { std::mem::uninitialized() };
    flag
}
```

**Correct (MaybeUninit tracks initialization state explicitly):**

```rust
use std::mem::MaybeUninit;

fn initialized_u32() -> u32 {
    // Step 1: Allocate space without asserting initialization.
    let mut uninit: MaybeUninit<u32> = MaybeUninit::uninit();

    // Step 2: Write a valid value before reading.
    uninit.write(42);

    // Step 3: Declare the value initialized and extract it.
    // SAFETY: We called write(42) immediately above, so the value is
    // fully initialized and 42 is a valid u32 bit pattern.
    unsafe { uninit.assume_init() }
}

fn build_array_lazily() -> [u32; 4] {
    let mut arr: [MaybeUninit<u32>; 4] = [const { MaybeUninit::uninit() }; 4];

    // Initialize each element individually — useful when initialization
    // depends on runtime data or fallible operations.
    for (i, slot) in arr.iter_mut().enumerate() {
        slot.write(i as u32 * 10);
    }

    // SAFETY: Every element was written in the loop above.
    // The loop covers the full range 0..4, so no slot is left uninitialized.
    arr.map(|x| unsafe { x.assume_init() })
}

fn maybe_initialized() -> Option<String> {
    let mut slot: MaybeUninit<String> = MaybeUninit::uninit();

    let should_init = true; // runtime condition
    if should_init {
        slot.write(String::from("hello"));
        // SAFETY: We entered the branch that calls write(), so the slot
        // contains a valid, fully initialized String.
        Some(unsafe { slot.assume_init() })
    } else {
        // The slot is never read — no UB even though it was never written.
        None
    }
}
```

**Note:** Use `[const { MaybeUninit::uninit() }; N]` (stable since Rust 1.79) to create arrays of `MaybeUninit` without nightly features. `MaybeUninit::uninit_array()` remains unstable as of early 2026. When you need to initialize a large buffer that will be filled entirely before use (e.g., from a `Read` impl), `MaybeUninit` avoids the cost of zeroing memory that will be immediately overwritten. The key discipline is: never call `assume_init()` unless every byte of the value has been written through the `MaybeUninit` API.
