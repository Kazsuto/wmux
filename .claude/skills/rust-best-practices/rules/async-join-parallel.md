---
title: Use tokio::join! for Concurrent Independent Futures
impact: HIGH
impactDescription: 2-10x improvement by running independent operations concurrently
tags: async, join, parallel, concurrency, tokio
---

## Use tokio::join! for Concurrent Independent Futures

Sequential .await on independent operations wastes time. Use tokio::join! to poll them concurrently on the same task.

**Incorrect (sequential awaiting — total latency is the sum of all durations):**

```rust
use std::time::Duration;
use tokio::time::sleep;

struct User { name: String }
struct Order { id: u64 }
struct Inventory { items: Vec<String> }

async fn fetch_users() -> Vec<User> {
    sleep(Duration::from_millis(100)).await;
    vec![User { name: "Alice".to_string() }]
}

async fn fetch_orders() -> Vec<Order> {
    sleep(Duration::from_millis(80)).await;
    vec![Order { id: 1 }]
}

async fn fetch_inventory() -> Inventory {
    sleep(Duration::from_millis(120)).await;
    Inventory { items: vec!["widget".to_string()] }
}

async fn load_dashboard_slow() {
    // Total: ~300ms — each operation waits for the previous to complete
    let users = fetch_users().await;
    let orders = fetch_orders().await;
    let inventory = fetch_inventory().await;

    println!("{} users, {} orders, {} items",
        users.len(), orders.len(), inventory.items.len());
}
```

**Correct (concurrent execution — total latency is the maximum single duration):**

```rust
use std::time::Duration;
use tokio::time::sleep;

struct User { name: String }
struct Order { id: u64 }
struct Inventory { items: Vec<String> }

async fn fetch_users() -> Vec<User> {
    sleep(Duration::from_millis(100)).await;
    vec![User { name: "Alice".to_string() }]
}

async fn fetch_orders() -> Vec<Order> {
    sleep(Duration::from_millis(80)).await;
    vec![Order { id: 1 }]
}

async fn fetch_inventory() -> Inventory {
    sleep(Duration::from_millis(120)).await;
    Inventory { items: vec!["widget".to_string()] }
}

async fn load_dashboard_fast() {
    // Total: ~120ms — all three run concurrently, wait for the slowest
    let (users, orders, inventory) = tokio::join!(
        fetch_users(),
        fetch_orders(),
        fetch_inventory(),
    );

    println!("{} users, {} orders, {} items",
        users.len(), orders.len(), inventory.items.len());
}

// For Result-returning futures, use try_join!
async fn load_dashboard_fallible() -> Result<(), String> {
    async fn fetch_users_r() -> Result<Vec<User>, String> {
        Ok(vec![User { name: "Alice".to_string() }])
    }
    async fn fetch_orders_r() -> Result<Vec<Order>, String> {
        Ok(vec![Order { id: 1 }])
    }

    // Cancels the remaining future immediately if either returns Err
    let (users, orders) = tokio::try_join!(
        fetch_users_r(),
        fetch_orders_r(),
    )?;

    println!("{} users, {} orders", users.len(), orders.len());
    Ok(())
}
```

**Note:** Use try_join! for Result-returning futures. Be aware that try_join! cancels all remaining futures as soon as one returns an error — this is usually desirable but ensure the cancellation is safe for your use case.
