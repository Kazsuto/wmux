---
title: Choose the Right Smart Pointer
impact: CRITICAL
impactDescription: wrong pointer type adds unnecessary overhead from atomic operations (2-5x per clone/drop uncontended, more under contention)
tags: memory, box, rc, arc, smart-pointers
---

## Choose the Right Smart Pointer

`Box` = single owner, no ref-count. `Rc` = multiple owners, single thread, non-atomic. `Arc` = multiple owners, multi-thread, atomic. Use the cheapest type that satisfies your requirements.

**Incorrect (pays atomic ref-count overhead in single-threaded code):**

```rust
use std::sync::Arc;

#[derive(Debug)]
struct Config {
    max_connections: u32,
    timeout_ms: u64,
}

fn build_handlers(config: Arc<Config>) -> Vec<Arc<Config>> {
    // Arc uses atomic operations for every clone and drop —
    // completely unnecessary when all handlers run on one thread
    vec![
        Arc::clone(&config),
        Arc::clone(&config),
        Arc::clone(&config),
    ]
}

fn main() {
    let config = Arc::new(Config { max_connections: 100, timeout_ms: 5000 });
    let handlers = build_handlers(config);
    println!("{} handlers", handlers.len());
}
```

**Correct (use Rc for single-threaded shared ownership; upgrade to Arc only at thread boundaries):**

```rust
use std::rc::Rc;
use std::sync::Arc;

#[derive(Debug)]
struct Config {
    max_connections: u32,
    timeout_ms: u64,
}

// Single-threaded code: Rc — non-atomic, no synchronization cost
fn build_handlers(config: Rc<Config>) -> Vec<Rc<Config>> {
    vec![
        Rc::clone(&config),
        Rc::clone(&config),
        Rc::clone(&config),
    ]
}

// Only when spawning threads do we pay for Arc
fn share_across_threads(config: Arc<Config>) {
    let c = Arc::clone(&config);
    std::thread::spawn(move || {
        println!("thread sees max_connections={}", c.max_connections);
    }).join().unwrap();
}

fn main() {
    let rc_config = Rc::new(Config { max_connections: 100, timeout_ms: 5000 });
    let handlers = build_handlers(rc_config);
    println!("{} handlers", handlers.len());

    let arc_config = Arc::new(Config { max_connections: 200, timeout_ms: 3000 });
    share_across_threads(arc_config);
}
```

**Note:** If single ownership suffices, skip reference counting entirely and use `Box` or a plain move. The hierarchy is: plain value (free) < `Box` (one allocation) < `Rc` (non-atomic ref-count) < `Arc` (atomic ref-count). Each step up adds overhead, so stop at the lowest rung that meets your requirements.
