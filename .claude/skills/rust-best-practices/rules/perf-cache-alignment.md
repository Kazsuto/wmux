---
title: Align Data Structures for Cache Efficiency
impact: MEDIUM
impactDescription: reduces cache misses and prevents false sharing
tags: performance, cache, alignment, repr-align, struct-layout
---

## Align Data Structures for Cache Efficiency

CPU cache lines are 64 bytes. Align hot data to cache line boundaries and keep frequently accessed fields together.

**Incorrect (false sharing between threads):**

```rust
use std::sync::atomic::{AtomicU64, Ordering};

// Both counters occupy the same 64-byte cache line.
// A write to `requests` invalidates the cache line for the thread
// reading `errors`, even though they are logically independent.
struct Metrics {
    requests: AtomicU64,
    errors: AtomicU64,
}

fn increment_request(m: &Metrics) {
    m.requests.fetch_add(1, Ordering::Relaxed);
}

fn increment_error(m: &Metrics) {
    m.errors.fetch_add(1, Ordering::Relaxed);
}
```

**Correct (separate cache lines per counter):**

```rust
use std::sync::atomic::{AtomicU64, Ordering};

// Each Padded<T> occupies exactly one 64-byte cache line.
// Threads writing to different counters no longer interfere.
#[repr(align(64))]
struct Padded<T>(T);

struct Metrics {
    requests: Padded<AtomicU64>,
    errors: Padded<AtomicU64>,
}

impl Metrics {
    fn new() -> Self {
        Self {
            requests: Padded(AtomicU64::new(0)),
            errors: Padded(AtomicU64::new(0)),
        }
    }

    fn increment_request(&self) {
        self.requests.0.fetch_add(1, Ordering::Relaxed);
    }

    fn increment_error(&self) {
        self.errors.0.fetch_add(1, Ordering::Relaxed);
    }
}
```

**Note:** Consider struct-of-arrays (SoA) layout for data-parallel processing where you iterate over one field millions of times. SoA keeps each field contiguous in memory, maximizing cache utilization during SIMD-style loops, compared to array-of-structs (AoS) which interleaves all fields.
