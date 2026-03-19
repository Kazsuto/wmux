---
title: Prefer std::sync::Mutex for Short Critical Sections in Async Code
impact: HIGH
impactDescription: lower overhead than tokio::sync::Mutex for brief locks
tags: concurrency, mutex, std, tokio, async
---

## Prefer std::sync::Mutex for Short Critical Sections in Async Code

The Tokio docs explicitly recommend std::sync::Mutex when the lock is held briefly and doesn't span .await points. tokio::sync::Mutex has higher task scheduling overhead.

**Incorrect (tokio::sync::Mutex for a trivial synchronous increment — unnecessary async overhead):**

```rust
use std::sync::Arc;
use tokio::sync::Mutex;

async fn increment_counter(counter: Arc<Mutex<u64>>) {
    // tokio::sync::Mutex involves the async task scheduler on every
    // lock/unlock — significant overhead for a nanosecond operation.
    let mut guard = counter.lock().await; // unnecessary .await
    *guard += 1;
    // Guard drops here
}

async fn run() {
    let counter = Arc::new(Mutex::new(0u64));

    // Spawning 1000 tasks, each paying tokio scheduler overhead for a u64 add
    let tasks: Vec<_> = (0..1000).map(|_| {
        let c = Arc::clone(&counter);
        tokio::spawn(async move { increment_counter(c).await })
    }).collect();

    for t in tasks { t.await.unwrap(); }
    println!("count: {}", *counter.lock().await);
}
```

**Correct (std::sync::Mutex for short critical sections; tokio::sync::Mutex only across .await):**

```rust
use std::sync::{Arc, Mutex};

async fn increment_counter(counter: Arc<Mutex<u64>>) {
    // std::sync::Mutex: no async overhead, lock/unlock in nanoseconds.
    // Safe because we do NOT hold the lock across an .await point.
    let mut guard = counter.lock().unwrap();
    *guard += 1;
    // Guard dropped immediately here — lock held for ~1ns
}

async fn run() {
    let counter = Arc::new(Mutex::new(0u64));

    let tasks: Vec<_> = (0..1000).map(|_| {
        let c = Arc::clone(&counter);
        tokio::spawn(async move { increment_counter(c).await })
    }).collect();

    for t in tasks { t.await.unwrap(); }
    println!("count: {}", *counter.lock().unwrap());
}

// When you MUST hold a lock across an .await, use tokio::sync::Mutex
async fn update_with_io(shared: Arc<tokio::sync::Mutex<String>>) {
    let mut guard = shared.lock().await;
    // Performing async I/O while holding the lock — tokio::sync::Mutex is correct here
    let data = tokio::fs::read_to_string("data.txt").await.unwrap();
    *guard = data;
    // Guard dropped after the .await completes
}
```

**Note:** Never hold std::sync::Mutex across an .await point. If a task suspends while holding a std Mutex, another task scheduled on the same OS thread will attempt to acquire the same mutex and deadlock. The compiler will warn about this in many cases, but it is not always caught statically.
