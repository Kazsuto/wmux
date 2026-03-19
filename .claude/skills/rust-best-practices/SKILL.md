---
name: rust-best-practices
description: Rust performance, safety, and idiomatic coding guidelines. Use when writing, reviewing, or refactoring Rust code to ensure optimal patterns for ownership, memory management, async, concurrency, error handling, type system, and testing.
---

# Rust Best Practices

Comprehensive performance and idiomatic coding guide for Rust applications. Contains 56 rules across 11 categories, prioritized by impact to guide automated refactoring and code generation.

## When to Apply

Reference these guidelines when:

- Writing new Rust modules, structs, or functions
- Implementing async code with Tokio or other runtimes
- Reviewing code for performance, safety, or idiomatic issues
- Refactoring existing Rust code
- Designing public APIs or library interfaces
- Writing concurrent or parallel code
- Handling errors or designing type hierarchies

## Rule Categories by Priority

| Priority | Category                    | Impact      | Prefix          |
| -------- | --------------------------- | ----------- | --------------- |
| 1        | Ownership & Borrowing       | CRITICAL    | `ownership-`    |
| 2        | Memory Management           | CRITICAL    | `memory-`       |
| 3        | Async Runtime               | CRITICAL    | `async-`        |
| 4        | Error Handling              | CRITICAL    | `error-`        |
| 5        | Unsafe Code                 | CRITICAL    | `unsafe-`       |
| 6        | Type System & Traits        | HIGH        | `type-`         |
| 7        | Concurrency & Parallelism   | HIGH        | `concurrency-`  |
| 8        | Lifetime Patterns           | HIGH        | `lifetime-`     |
| 9        | Performance Optimization    | HIGH        | `perf-`         |
| 10       | Compilation & Binary        | MEDIUM      | `compile-`      |
| 11       | Iterator Patterns           | MEDIUM      | `iterator-`     |
| 12       | Testing Patterns            | MEDIUM      | `testing-`      |

## Quick Reference

### 1. Ownership & Borrowing (CRITICAL)

- `ownership-accept-slices` - Accept &str/&[T] not &String/&Vec<T>
- `ownership-avoid-clone` - Avoid unnecessary .clone(), borrow instead
- `ownership-cow-conditional` - Use Cow for conditional modification
- `ownership-into-string` - Accept impl Into<String> when ownership needed
- `ownership-clone-from` - Use clone_from instead of x = y.clone()

### 2. Memory Management (CRITICAL)

- `memory-stack-allocation` - Prefer stack allocation; Box only when needed
- `memory-smart-pointers` - Choose the right smart pointer: Box vs Rc vs Arc
- `memory-reuse-allocations` - Reuse allocations in hot loops
- `memory-vec-capacity` - Pre-allocate Vec with with_capacity
- `memory-smallvec` - Use SmallVec for small, short-lived collections
- `memory-arena-allocator` - Use arena allocators for same-lifetime objects

### 3. Async Runtime (CRITICAL)

- `async-no-blocking` - Never block the async runtime
- `async-cancellation-safety` - Understand cancellation safety with select!
- `async-futurelock` - Avoid FutureLock (resource contention in select!)
- `async-join-parallel` - Use tokio::join! for concurrent independent futures
- `async-send-bounds` - Add Send bounds to async trait methods
- `async-bounded-channels` - Use bounded channels for backpressure

### 4. Error Handling (CRITICAL)

- `error-thiserror-anyhow` - thiserror for libraries, anyhow for applications
- `error-context` - Add context to errors with .context()
- `error-expect-messages` - Use expect() over unwrap() with invariant messages
- `error-no-unit-type` - Never use () as an error type

### 5. Unsafe Code (CRITICAL)

- `unsafe-safe-abstraction` - Encapsulate unsafe behind safe APIs with SAFETY docs
- `unsafe-ffi-raii` - Wrap FFI in RAII-based safe abstractions
- `unsafe-maybeuninit` - Use MaybeUninit instead of mem::uninitialized

### 6. Type System & Traits (HIGH)

- `type-newtype` - Newtype pattern for type safety
- `type-typestate` - Typestate pattern for compile-time state enforcement
- `type-static-dispatch` - Prefer impl Trait (static dispatch) by default
- `type-common-traits` - Eagerly implement common standard traits
- `type-from-not-into` - Implement From, not Into
- `type-sealed-trait` - Sealed trait pattern for forward compatibility
- `type-builder-pattern` - Builder pattern for complex construction
- `type-extension-traits` - Extension traits for foreign types

### 7. Concurrency & Parallelism (HIGH)

- `concurrency-std-mutex` - Prefer std::sync::Mutex for short critical sections
- `concurrency-actor-pattern` - Use channels (actor pattern) over Arc<Mutex<T>>
- `concurrency-rwlock` - Prefer RwLock only when reads dominate
- `concurrency-false-sharing` - Pad per-thread atomics to avoid false sharing
- `concurrency-rayon` - Use Rayon par_iter() for CPU-bound work
- `concurrency-rayon-granularity` - Tune Rayon granularity with with_min_len

### 8. Lifetime Patterns (HIGH)

- `lifetime-elision-rules` - Know the three elision rules; omit redundant annotations
- `lifetime-struct-references` - Structs with references need explicit lifetimes
- `lifetime-no-local-refs` - Never return references to local variables
- `lifetime-static-bounds` - Use 'static deliberately, not as an escape hatch
- `lifetime-hrtb` - Use HRTB (for<'a>) for lifetime-generic callbacks

### 9. Performance Optimization (HIGH)

- `perf-inline` - Use #[inline] strategically for cross-crate functions
- `perf-cache-alignment` - Align data structures for cache efficiency
- `perf-benchmarking` - Benchmark with Criterion and black_box
- `perf-small-strings` - Use small string optimization for short strings

### 10. Compilation & Binary (MEDIUM)

- `compile-release-profile` - Configure release profile for max performance
- `compile-feature-flags` - Use feature flags for optional dependencies

### 11. Iterator Patterns (MEDIUM)

- `iterator-avoid-collect` - Avoid needless_collect, keep iterators lazy
- `iterator-ownership` - Choose iter/into_iter/iter_mut by ownership need
- `iterator-combinators` - Prefer Option/Result combinators over nested match

### 12. Testing Patterns (MEDIUM)

- `testing-organization` - Organize tests: #[cfg(test)] modules + tests/ dir
- `testing-property-based` - Property-based testing with proptest
- `testing-dependency-injection` - Trait-based dependency injection for testable code
- `testing-doc-tests` - Doc tests to keep examples honest

## How to Use

Read individual rule files for detailed explanations and code examples:

```
rules/ownership-accept-slices.md
rules/async-no-blocking.md
```

Each rule file contains:

- Brief explanation of why it matters
- Incorrect code example with explanation
- Correct code example with explanation
- Additional context and references
