---
title: Use Channels (Actor Pattern) Over Arc<Mutex<T>>
impact: HIGH
impactDescription: eliminates lock contention and simplifies concurrent state management
tags: concurrency, actor, channels, mpsc, oneshot, mutex
---

## Use Channels (Actor Pattern) Over Arc<Mutex<T>>

The actor pattern uses channels to serialize access to shared state. One task owns the data exclusively and processes messages sequentially — no locks needed.

**Incorrect (multiple tasks fighting over Arc<Mutex<HashMap>> — contention and manual guard drops):**

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

async fn cache_reader(cache: Arc<Mutex<HashMap<String, String>>>, key: String) -> Option<String> {
    // Lock contention: every reader and writer blocks every other
    let guard = cache.lock().await;
    guard.get(&key).cloned()
    // Must remember to drop guard before any .await
}

async fn cache_writer(cache: Arc<Mutex<HashMap<String, String>>>, key: String, val: String) {
    let mut guard = cache.lock().await;
    guard.insert(key, val);
    // Forgetting to drop guard before an .await here would be a bug
}

async fn run_bad() {
    let cache: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));

    // Multiple spawned tasks all contend for the same lock
    let c1 = Arc::clone(&cache);
    let c2 = Arc::clone(&cache);
    tokio::join!(
        tokio::spawn(async move { cache_reader(c1, "key".to_string()).await }),
        tokio::spawn(async move { cache_writer(c2, "key".to_string(), "val".to_string()).await }),
    );
}
```

**Correct (actor task owns HashMap exclusively; callers communicate via mpsc and oneshot):**

```rust
use std::collections::HashMap;
use tokio::sync::{mpsc, oneshot};

// Messages the actor understands
enum CacheCommand {
    Get {
        key: String,
        reply: oneshot::Sender<Option<String>>,
    },
    Set {
        key: String,
        value: String,
    },
}

// The actor: owns all state, processes messages sequentially
async fn cache_actor(mut rx: mpsc::Receiver<CacheCommand>) {
    let mut map: HashMap<String, String> = HashMap::new();

    while let Some(cmd) = rx.recv().await {
        match cmd {
            CacheCommand::Get { key, reply } => {
                let _ = reply.send(map.get(&key).cloned());
            }
            CacheCommand::Set { key, value } => {
                map.insert(key, value);
            }
        }
    }
}

// Handle: cheap to clone, safe to share across tasks
#[derive(Clone)]
struct CacheHandle {
    tx: mpsc::Sender<CacheCommand>,
}

impl CacheHandle {
    fn new() -> (Self, mpsc::Receiver<CacheCommand>) {
        let (tx, rx) = mpsc::channel(64); // bounded for backpressure
        (Self { tx }, rx)
    }

    async fn get(&self, key: &str) -> Option<String> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx.send(CacheCommand::Get {
            key: key.to_string(),
            reply: reply_tx,
        }).await.ok()?;
        reply_rx.await.ok()?
    }

    async fn set(&self, key: &str, value: String) {
        let _ = self.tx.send(CacheCommand::Set {
            key: key.to_string(),
            value,
        }).await;
    }
}

async fn run_good() {
    let (handle, rx) = CacheHandle::new();

    // Spawn the actor — it owns the HashMap exclusively
    tokio::spawn(cache_actor(rx));

    // Multiple handles share access without any locks
    let h1 = handle.clone();
    let h2 = handle.clone();
    tokio::join!(
        async move { h1.set("key", "value".to_string()).await },
        async move { h2.get("key").await },
    );
}
```

**Note:** This is Alice Ryhl's recommended pattern for shared async state. Bounded channels provide natural backpressure and enable graceful shutdown — when all CacheHandle senders are dropped, cache_actor's rx.recv() returns None and the task exits cleanly.
