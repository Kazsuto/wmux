---
title: Use Bounded Channels for Backpressure
impact: HIGH
impactDescription: prevents memory exhaustion under sustained load
tags: async, channels, backpressure, mpsc, memory
---

## Use Bounded Channels for Backpressure

Unbounded channels grow without limit if the producer is faster than the consumer. Bounded channels apply natural backpressure by suspending the sender when full.

**Incorrect (unbounded channel — no backpressure, unbounded memory growth):**

```rust
use tokio::sync::mpsc;

async fn start_pipeline_unbounded() {
    // No capacity limit — if the consumer is slow, messages accumulate
    // indefinitely in memory until the process is OOM-killed.
    let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();

    let producer = tokio::spawn(async move {
        loop {
            // Producer never slows down regardless of consumer speed
            let data = vec![0u8; 4096];
            tx.send(data).unwrap(); // Never blocks or fails (until OOM)
        }
    });

    let consumer = tokio::spawn(async move {
        while let Some(data) = rx.recv().await {
            // Simulate slow consumer
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            let _ = data;
        }
    });

    let _ = tokio::join!(producer, consumer);
}
```

**Correct (bounded channel — sender suspends when full, preventing memory exhaustion):**

```rust
use tokio::sync::mpsc;

async fn start_pipeline_bounded() {
    // Bounded capacity: sender will .await when the channel is full,
    // automatically slowing the producer to match consumer throughput.
    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(1000);

    let producer = tokio::spawn(async move {
        loop {
            let data = vec![0u8; 4096];
            // send().await suspends this task when the channel is full —
            // no memory exhaustion, natural backpressure.
            if tx.send(data).await.is_err() {
                break; // Receiver dropped — shut down cleanly
            }
        }
    });

    let consumer = tokio::spawn(async move {
        while let Some(data) = rx.recv().await {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            let _ = data;
        }
    });

    let _ = tokio::join!(producer, consumer);
}

// try_send for non-blocking producers that handle backpressure explicitly
async fn non_blocking_producer(tx: mpsc::Sender<String>) {
    let msg = "event".to_string();
    match tx.try_send(msg) {
        Ok(()) => {} // Accepted
        Err(mpsc::error::TrySendError::Full(_)) => {
            // Channel full — apply custom backpressure logic (drop, sample, retry)
        }
        Err(mpsc::error::TrySendError::Closed(_)) => {
            // Receiver gone — shut down
        }
    }
}
```

**Note:** Size 32–64 works well for actor patterns where the consumer processes one message at a time. Use larger sizes (256–4096) to smooth over burst traffic while still bounding peak memory. Never use unbounded_channel in production code on a hot path.
