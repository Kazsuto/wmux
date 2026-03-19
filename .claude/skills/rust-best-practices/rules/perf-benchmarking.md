---
title: Benchmark with Criterion and black_box
impact: HIGH
impactDescription: produces statistically rigorous, non-optimized-away benchmarks
tags: performance, benchmark, criterion, black-box, testing
---

## Benchmark with Criterion and black_box

Use `criterion` for statistically rigorous benchmarks. Always use `std::hint::black_box` to prevent the compiler from optimizing away the code under test.

**Incorrect (compiler may eliminate the call entirely):**

```rust
// benches/fibonacci.rs
use criterion::{criterion_group, criterion_main, Criterion};

fn fibonacci(n: u64) -> u64 {
    match n {
        0 => 0,
        1 => 1,
        n => fibonacci(n - 1) + fibonacci(n - 2),
    }
}

fn bench_fibonacci(c: &mut Criterion) {
    c.bench_function("fibonacci 20", |b| {
        // Without black_box: the compiler can treat 20 as a known constant,
        // specialize the call, and discard the unused return value as dead code —
        // reducing the loop body to a no-op.
        b.iter(|| fibonacci(20))
    });
}

criterion_group!(benches, bench_fibonacci);
criterion_main!(benches);
```

**Correct (black_box prevents constant-folding and dead code elimination):**

```rust
// benches/fibonacci.rs
use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;

fn fibonacci(n: u64) -> u64 {
    match n {
        0 => 0,
        1 => 1,
        n => fibonacci(n - 1) + fibonacci(n - 2),
    }
}

fn bench_fibonacci(c: &mut Criterion) {
    c.bench_function("fibonacci 20", |b| {
        // black_box on the input prevents the compiler from treating
        // it as a constant. black_box on the output prevents the
        // result from being discarded as unused.
        b.iter(|| black_box(fibonacci(black_box(20))))
    });
}

criterion_group!(benches, bench_fibonacci);
criterion_main!(benches);
```

```toml
# Cargo.toml
[dev-dependencies]
criterion = { version = "0.8", features = ["html_reports"] }

[[bench]]
name = "fibonacci"
harness = false
```

**Note:** Alternatives worth considering: `divan` (simpler annotation-based syntax, less boilerplate), `iai-callgrind` (measures instruction counts instead of wall time — deterministic and CI-friendly, unaffected by system load).
