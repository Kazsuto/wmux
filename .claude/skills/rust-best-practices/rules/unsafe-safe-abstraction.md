---
title: Encapsulate Unsafe Behind Safe APIs with SAFETY Docs
impact: CRITICAL
impactDescription: confines undefined behavior risk to auditable boundaries
tags: unsafe, safety, encapsulation, abstraction, documentation
---

## Encapsulate Unsafe Behind Safe APIs with SAFETY Docs

Every `unsafe` block must have a `// SAFETY:` comment. Every `unsafe fn` must have a `# Safety` doc section. Expose only safe public APIs.

**Incorrect (unsafe leaks to all callers):**

```rust
// The `unsafe` qualifier propagates to every call site.
// Every caller must now reason about pointer validity and bounds —
// this is a maintenance and audit burden that compounds with each new caller.
pub unsafe fn get_value(ptr: *const i32, index: usize) -> i32 {
    // No SAFETY comment — future readers cannot verify the invariants.
    *ptr.add(index)
}

fn main() {
    let data = [10_i32, 20, 30];
    // Every call site is an unsafe block with no documented justification.
    let val = unsafe { get_value(data.as_ptr(), 1) };
    println!("{val}");
}
```

**Correct (unsafe confined inside a safe abstraction):**

```rust
use std::ptr::NonNull;

/// A bounds-checked view into a contiguous slice of `i32` values.
pub struct IntSlice<'a> {
    ptr: NonNull<i32>,
    len: usize,
    _marker: std::marker::PhantomData<&'a [i32]>,
}

impl<'a> IntSlice<'a> {
    /// Creates a new `IntSlice` from a slice reference.
    pub fn new(slice: &'a [i32]) -> Self {
        // SAFETY: slice.as_ptr() is guaranteed non-null because it comes from
        // a valid shared reference. NonNull::new_unchecked is safe here.
        let ptr = unsafe { NonNull::new_unchecked(slice.as_ptr() as *mut i32) };
        Self { ptr, len: slice.len(), _marker: std::marker::PhantomData }
    }

    /// Returns the element at `index`, or `None` if out of bounds.
    ///
    /// Bounds checking is performed before any pointer arithmetic.
    /// All unsafe invariants are upheld internally.
    pub fn get(&self, index: usize) -> Option<i32> {
        if index >= self.len {
            return None;
        }
        // SAFETY: `index` is verified to be less than `self.len`, which equals
        // the length of the original slice. The pointer was obtained from a valid
        // reference, so it is properly aligned and points to initialized memory.
        Some(unsafe { *self.ptr.as_ptr().add(index) })
    }
}

fn main() {
    let data = [10_i32, 20, 30];
    let slice = IntSlice::new(&data);

    // Callers use a fully safe API — no unsafe at the call site.
    assert_eq!(slice.get(1), Some(20));
    assert_eq!(slice.get(5), None);
}
```

**Note:** Run `cargo +nightly miri test` to detect undefined behavior at test time. Miri interprets your code and catches issues like use-after-free, out-of-bounds pointer arithmetic, and invalid memory reads that the compiler's static analysis cannot find. Audit `unsafe` blocks by searching for `unsafe` without a following `// SAFETY:` comment — making this a CI lint rule keeps the codebase honest.
