---
title: Use Feature Flags for Optional Dependencies
impact: MEDIUM
impactDescription: reduces compile times and binary size for consumers
tags: compilation, features, conditional, dependencies, cargo
---

## Use Feature Flags for Optional Dependencies

Make heavy dependencies optional with Cargo feature flags. Use `#[cfg(feature = "...")]` for conditional compilation.

**Incorrect (all dependencies always compiled):**

```toml
# Cargo.toml — every consumer compiles everything,
# even if they only need the core functionality.
[package]
name = "mylib"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
reqwest = { version = "0.12", features = ["json"] }
tokio = { version = "1", features = ["full"] }
```

```rust
// src/lib.rs — always compiled even when caller doesn't need HTTP or JSON
use serde::{Deserialize, Serialize};
use reqwest::Client;

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub name: String,
    pub value: u32,
}

pub async fn fetch_config(url: &str) -> Result<Config, reqwest::Error> {
    Client::new().get(url).send().await?.json().await
}
```

**Correct (optional dependencies gated behind feature flags):**

```toml
# Cargo.toml
[package]
name = "mylib"
version = "0.1.0"
edition = "2021"

[features]
default = []
# Use `dep:` prefix to avoid the feature implicitly enabling the dependency.
serialization = ["dep:serde", "dep:serde_json"]
http = ["dep:reqwest", "dep:tokio"]

[dependencies]
serde = { version = "1", features = ["derive"], optional = true }
serde_json = { version = "1", optional = true }
reqwest = { version = "0.12", features = ["json"], optional = true }
tokio = { version = "1", features = ["rt-multi-thread"], optional = true }
```

```rust
// src/lib.rs — heavy code only compiled when the feature is enabled

#[cfg_attr(feature = "serialization", derive(serde::Serialize, serde::Deserialize))]
pub struct Config {
    pub name: String,
    pub value: u32,
}

#[cfg(feature = "serialization")]
impl Config {
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
}

#[cfg(feature = "http")]
pub async fn fetch_config(url: &str) -> Result<Config, reqwest::Error> {
    reqwest::Client::new()
        .get(url)
        .send()
        .await?
        .json()
        .await
}
```

```toml
# Consumer Cargo.toml — only pay for what you use
[dependencies]
mylib = { version = "1", features = ["serialization"] }
# reqwest and tokio are NOT compiled for this consumer
```

**Note:** In workspaces, use `[workspace.dependencies]` to declare shared dependency versions once and reference them with `{ workspace = true }` in member crates. This avoids version drift and reduces resolver work. Avoid adding features to `default` unless the functionality is needed by the vast majority of users — surprising compile-time costs erode trust in a library.
