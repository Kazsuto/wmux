---
title: Tune Rayon Granularity with with_min_len
impact: MEDIUM
impactDescription: prevents parallelism overhead from exceeding computation cost
tags: concurrency, rayon, granularity, min-len, performance
---

## Tune Rayon Granularity with with_min_len

For cheap operations, Rayon's splitting overhead can exceed the work itself. Set with_min_len to amortize scheduling overhead, or skip parallelism entirely for small inputs.

**Incorrect (par_iter() on trivially cheap ops — overhead dominates, slower than sequential):**

```rust
use rayon::prelude::*;

fn sum_trivial_bad(numbers: &[u64]) -> u64 {
    // For a simple integer sum, each element takes ~1ns.
    // Rayon splits the work into chunks and schedules them across threads.
    // Thread pool overhead (~1-10us per task) far exceeds the actual work
    // for small-to-medium inputs.
    numbers.par_iter().sum()
}

fn parse_bad(strings: &[&str]) -> Vec<u64> {
    // Parsing a short integer string is ~50ns — cheap, but not free.
    // Without min_len, Rayon creates O(num_threads * log N) tasks,
    // each handling just a handful of elements.
    strings.par_iter().filter_map(|s| s.parse().ok()).collect()
}

fn expensive_bad(data: &[Vec<u8>]) -> Vec<usize> {
    // Even expensive operations should skip parallelism for tiny inputs
    if data.len() < 4 {
        // At least guard against tiny inputs
    }
    data.par_iter().map(|chunk| chunk.iter().filter(|&&b| b > 128).count()).collect()
}
```

**Correct (with_min_len calibrated to operation cost; skip parallelism for small inputs):**

```rust
use rayon::prelude::*;

// Cheap ops (integer arithmetic, array access): min_len 10K-100K
fn sum_cheap_correct(numbers: &[u64]) -> u64 {
    if numbers.len() < 10_000 {
        return numbers.iter().sum(); // Sequential is faster below threshold
    }
    numbers
        .par_iter()
        .with_min_len(10_000) // Each chunk does ≥10K additions (~10us of work)
        .sum()
}

// Moderate ops (string parsing, small allocations): min_len 100-1K
fn parse_moderate_correct(strings: &[&str]) -> Vec<u64> {
    if strings.len() < 500 {
        return strings.iter().filter_map(|s| s.parse().ok()).collect();
    }
    strings
        .par_iter()
        .with_min_len(100) // Each chunk parses ≥100 strings (~5us of work)
        .filter_map(|s| s.parse().ok())
        .collect()
}

// Expensive ops (crypto, compression, image processing): min_len 1-10
fn hash_expensive_correct(chunks: &[Vec<u8>]) -> Vec<[u8; 32]> {
    // Each SHA-256 call takes ~50-200us — parallelism overhead is negligible
    chunks
        .par_iter()
        .with_min_len(1) // Every single item justifies its own parallel task
        .map(|data| {
            // Simulate expensive per-item work
            let mut hash = [0u8; 32];
            hash[0] = data.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
            hash
        })
        .collect()
}
```

**Note:** When in doubt, benchmark with criterion. The crossover point where par_iter() beats sequential iteration depends on the operation cost, input size, and available cores. A conservative rule: if the total work takes less than ~1ms, sequential iteration is likely faster.
