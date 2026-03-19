---
title: Never Block the Async Runtime
impact: CRITICAL
impactDescription: prevents latency spikes and deadlocks across all tasks
tags: async, tokio, blocking, spawn-blocking, runtime
---

## Never Block the Async Runtime

Blocking operations inside async functions prevent the executor from polling other tasks. Use tokio async equivalents or spawn_blocking for any work that is not instantly non-blocking.

**Incorrect (blocking calls inside async fn stall the entire thread):**

```rust
use std::time::Duration;

async fn handle_request(path: &str) -> String {
    // Blocks the entire Tokio worker thread — no other tasks can run
    std::thread::sleep(Duration::from_millis(100));

    // Synchronous file I/O blocks the thread
    let contents = std::fs::read_to_string(path).unwrap();

    // CPU-heavy work on the async thread
    let hash = bcrypt::hash("password", 12).unwrap();

    format!("{} {}", contents.len(), hash)
}
```

**Correct (async equivalents and spawn_blocking for CPU-bound work):**

```rust
use std::time::Duration;
use tokio::time;
use tokio::fs;

async fn handle_request(path: &str) -> String {
    // Non-blocking sleep — yields back to the executor
    time::sleep(Duration::from_millis(100)).await;

    // Async file I/O — does not block the worker thread
    let contents = fs::read_to_string(path).await.unwrap();

    // Offload CPU-heavy work to a dedicated blocking thread pool
    let hash = tokio::task::spawn_blocking(|| {
        bcrypt::hash("password", 12).unwrap()
    })
    .await
    .unwrap();

    format!("{} {}", contents.len(), hash)
}
```

**Note:** Use spawn_blocking for work taking more than 10–100 microseconds. For indefinitely blocking work (e.g., waiting on a synchronous library), prefer std::thread::spawn with a channel to communicate results back to the async world.
