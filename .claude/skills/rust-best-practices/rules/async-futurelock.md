---
title: Avoid FutureLock — Resource Contention in select!
impact: CRITICAL
impactDescription: prevents silent deadlocks in async task scheduling
tags: async, select, deadlock, futurelock, mutex
---

## Avoid FutureLock — Resource Contention in select!

Multiple futures in select! competing for the same Mutex cause deadlocks because select! polls all branches and one may hold a lock while another tries to acquire it.

**Incorrect (two branches contend for the same Mutex — deadlock):**

```rust
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Default)]
struct SharedState {
    count: u64,
    log: Vec<String>,
}

async fn process(shared: Arc<Mutex<SharedState>>, mut rx: tokio::sync::mpsc::Receiver<String>) {
    loop {
        tokio::select! {
            // Branch A acquires the lock
            _ = async {
                let mut s = shared.lock().await;
                s.count += 1;
            } => {}

            // Branch B also tries to acquire the SAME lock.
            // If branch A holds the lock and branch B is polled,
            // the runtime deadlocks: A can't complete without the executor
            // polling other things, but B is blocking the executor.
            msg = rx.recv() => {
                if let Some(m) = msg {
                    let mut s = shared.lock().await;
                    s.log.push(m);
                }
            }
        }
    }
}
```

**Correct (use channels to signal between branches; acquire lock only once per iteration):**

```rust
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

#[derive(Default)]
struct SharedState {
    count: u64,
    log: Vec<String>,
}

enum Command {
    Increment,
    Log(String),
}

async fn process(shared: Arc<Mutex<SharedState>>, mut rx: mpsc::Receiver<Command>) {
    // Only one branch, one lock acquisition per loop iteration.
    // select! chooses among independent event sources, not shared resources.
    while let Some(cmd) = rx.recv().await {
        let mut s = shared.lock().await;
        match cmd {
            Command::Increment => s.count += 1,
            Command::Log(msg) => s.log.push(msg),
        }
        // Lock is dropped here before the next iteration
    }
}

// Separate tasks send commands over the channel — no direct lock access
async fn producer(tx: mpsc::Sender<Command>) {
    tx.send(Command::Increment).await.unwrap();
    tx.send(Command::Log("event".to_string())).await.unwrap();
}
```

**Note:** This pattern was discovered and documented by Oxide (referred to as "Futurelock"). The fix is to ensure at most one branch in a select! loop acquires any given resource. Prefer the actor pattern (see concurrency-actor-pattern.md) to eliminate shared locks entirely.
