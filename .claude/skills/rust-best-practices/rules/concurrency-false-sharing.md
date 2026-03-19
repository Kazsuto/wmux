---
title: Pad Per-Thread Atomics to Avoid False Sharing
impact: MEDIUM
impactDescription: prevents 10x+ performance degradation from cache line invalidation
tags: concurrency, false-sharing, cache-line, atomics, repr-align
---

## Pad Per-Thread Atomics to Avoid False Sharing

Adjacent atomics share CPU cache lines. One thread's write invalidates the cache for all other threads, even though no logical data is shared.

**Incorrect (all counters packed into one or two cache lines — every write invalidates neighbors):**

```rust
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

struct PerThreadCounters {
    // All 8 counters fit in 1-2 cache lines (8 * 8 bytes = 64 bytes).
    // When thread 0 writes counters[0], the CPU invalidates the entire
    // cache line on every other core — even threads 1-7 that don't touch [0].
    counters: [AtomicU64; 8],
}

fn run_false_sharing() {
    let counters = Arc::new(PerThreadCounters {
        counters: std::array::from_fn(|_| AtomicU64::new(0)),
    });

    let handles: Vec<_> = (0..8).map(|i| {
        let c = Arc::clone(&counters);
        thread::spawn(move || {
            for _ in 0..10_000_000 {
                // Each thread writes to "its own" counter, but all writes
                // bounce the same cache lines between cores.
                c.counters[i].fetch_add(1, Ordering::Relaxed);
            }
        })
    }).collect();

    handles.into_iter().for_each(|h| h.join().unwrap());
    let total: u64 = counters.counters.iter().map(|c| c.load(Ordering::Relaxed)).sum();
    println!("total: {total}");
}
```

**Correct (#[repr(align(64))] places each counter on its own cache line):**

```rust
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

// Each PaddedCounter occupies exactly one 64-byte cache line.
// Writes to one never invalidate another.
#[repr(align(64))]
struct PaddedCounter {
    value: AtomicU64,
    // Compiler inserts padding to fill the remaining 56 bytes
}

impl PaddedCounter {
    const fn new() -> Self {
        Self { value: AtomicU64::new(0) }
    }
}

fn run_no_false_sharing() {
    // Array of padded counters — each on its own cache line
    let counters: Arc<[PaddedCounter; 8]> = Arc::new(
        std::array::from_fn(|_| PaddedCounter::new())
    );

    let handles: Vec<_> = (0..8).map(|i| {
        let c = Arc::clone(&counters);
        thread::spawn(move || {
            for _ in 0..10_000_000 {
                // Thread i's writes are fully isolated — no cache line bouncing
                c[i].value.fetch_add(1, Ordering::Relaxed);
            }
        })
    }).collect();

    handles.into_iter().for_each(|h| h.join().unwrap());
    let total: u64 = counters.iter().map(|c| c.value.load(Ordering::Relaxed)).sum();
    println!("total: {total}");
}
```

**Note:** Always pad to cache line boundaries when multiple threads write to adjacent memory. On x86-64 and most ARM64 chips (e.g., AWS Graviton), the cache line is 64 bytes. Apple Silicon (M1/M2/M3/M4) uses 128-byte cache lines — use `#[repr(align(128))]` if targeting those processors. For maximum portability across all architectures, prefer 128-byte alignment. For critical inner loops, verify with a profiler (`perf c2c` on Linux) that false sharing is actually occurring before adding padding.
