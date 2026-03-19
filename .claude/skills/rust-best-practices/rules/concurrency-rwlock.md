---
title: Prefer RwLock Only When Reads Vastly Outnumber Writes
impact: MEDIUM
impactDescription: avoids unnecessary overhead for mixed workloads
tags: concurrency, rwlock, mutex, performance, contention
---

## Prefer RwLock Only When Reads Vastly Outnumber Writes

RwLock has higher per-operation overhead than Mutex. It only wins when reads dominate (>90%) and contention is high. For mixed workloads, Mutex is often faster.

**Incorrect (RwLock for a counter with roughly equal reads and writes — overhead exceeds the benefit):**

```rust
use std::sync::{Arc, RwLock};
use std::thread;

fn increment_and_read_rwlock(counter: Arc<RwLock<u64>>) {
    // 50% writes — write lock acquisition serializes everything anyway,
    // but with more overhead than a plain Mutex due to reader/writer tracking.
    {
        let mut w = counter.write().unwrap();
        *w += 1;
    }
    {
        let r = counter.read().unwrap();
        let _ = *r;
    }
}

fn run_bad() {
    let counter = Arc::new(RwLock::new(0u64));
    let handles: Vec<_> = (0..8).map(|_| {
        let c = Arc::clone(&counter);
        thread::spawn(move || {
            for _ in 0..10_000 {
                increment_and_read_rwlock(c.clone());
            }
        })
    }).collect();
    handles.into_iter().for_each(|h| h.join().unwrap());
}
```

**Correct (Mutex as the default; RwLock only when profiling confirms read-dominated contention):**

```rust
use std::sync::{Arc, Mutex, RwLock};
use std::collections::HashMap;
use std::thread;

// Default: Mutex for any mixed read/write workload
fn run_mutex_default() {
    let counter = Arc::new(Mutex::new(0u64));
    let handles: Vec<_> = (0..8).map(|_| {
        let c = Arc::clone(&counter);
        thread::spawn(move || {
            for _ in 0..10_000 {
                *c.lock().unwrap() += 1;
            }
        })
    }).collect();
    handles.into_iter().for_each(|h| h.join().unwrap());
}

// RwLock justified: configuration map loaded once, read millions of times
fn build_config_cache() -> Arc<RwLock<HashMap<String, String>>> {
    let cache = Arc::new(RwLock::new(HashMap::new()));
    {
        let mut w = cache.write().unwrap();
        w.insert("timeout".to_string(), "30s".to_string());
        w.insert("retries".to_string(), "3".to_string());
    }
    cache
    // From here on: ~99.9% reads, ~0.1% writes on config reload
    // RwLock allows all reader threads to proceed simultaneously
}

fn read_config(cache: &Arc<RwLock<HashMap<String, String>>>, key: &str) -> Option<String> {
    cache.read().unwrap().get(key).cloned()
}
```

**Note:** Consider lock-free alternatives like the arc-swap crate for read-heavy configuration swapping. arc-swap allows readers to proceed without any locking by atomically swapping an Arc pointer — ideal for config structs that are replaced wholesale rather than mutated in place.
