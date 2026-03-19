---
title: Add Send Bounds to Async Trait Methods
impact: HIGH
impactDescription: enables async trait usage with tokio::spawn on multi-threaded runtimes
tags: async, traits, send, tokio, trait-variant
---

## Add Send Bounds to Async Trait Methods

async fn in traits returns futures that are NOT Send by default. Public traits need Send bounds for multi-threaded runtime compatibility.

**Incorrect (future returned by async fn in trait is not Send — breaks with tokio::spawn):**

```rust
// This compiles, but the returned future is not Send.
// Any tokio::spawn call using this trait will fail to compile
// on a multi-threaded runtime.
pub trait DataStore {
    async fn get(&self, key: &str) -> Option<String>;
    async fn set(&self, key: &str, value: String);
}

struct RedisStore;

impl DataStore for RedisStore {
    async fn get(&self, key: &str) -> Option<String> {
        Some(format!("value_for_{key}"))
    }
    async fn set(&self, key: &str, value: String) {
        let _ = (key, value);
    }
}

// This fails to compile: `impl DataStore` cannot be sent between threads
// async fn use_store(store: impl DataStore) {
//     tokio::spawn(async move { store.get("key").await });
// }
```

**Correct (use trait_variant::make to generate a Send-bounded variant):**

```rust
// With the trait-variant crate (Rust 1.75+)
#[trait_variant::make(DataStore: Send)]
pub trait LocalDataStore {
    async fn get(&self, key: &str) -> Option<String>;
    async fn set(&self, key: &str, value: String);
}

struct RedisStore;

// Implement the Send variant — the future is now Send
impl DataStore for RedisStore {
    async fn get(&self, key: &str) -> Option<String> {
        Some(format!("value_for_{key}"))
    }
    async fn set(&self, key: &str, value: String) {
        let _ = (key, value);
    }
}

// Now this compiles correctly on a multi-threaded runtime
async fn use_store(store: impl DataStore + 'static) {
    tokio::spawn(async move {
        store.get("key").await;
    });
}

// Manual desugaring alternative (no extra crate dependency)
use std::future::Future;

pub trait DataStoreManual {
    fn get(&self, key: &str) -> impl Future<Output = Option<String>> + Send;
}
```

**Note:** Since Rust 1.75, native `async fn` in traits works for static dispatch without external crates. The `async-trait` crate (which boxes futures) is still needed for `dyn Trait` (object safety). Since Rust 1.85, async closures (`async || { ... }`) are stable, providing `AsyncFn`/`AsyncFnMut`/`AsyncFnOnce` traits. For `AsyncFnOnce`, the returned future is `Send` when all captures are `Send`. For `AsyncFn`/`AsyncFnMut`, the future also borrows `&self`/`&mut self`, so `Send`-ness depends on both the captures and the receiver type being `Send`. This reduces the need for `trait_variant` in callback-style APIs. Return Type Notation (RTN), allowing `T: Trait<method(..): Send>` at call sites, is still unstable as of early 2026. The `async_fn_in_trait` lint warns by default on public traits; suppress with `#[allow(async_fn_in_trait)]` when `Send` is not needed.
