---
title: Use thiserror for Libraries, anyhow for Applications
impact: CRITICAL
impactDescription: enables typed error matching in libraries and ergonomic propagation in apps
tags: error, thiserror, anyhow, library, application
---

## Use thiserror for Libraries, anyhow for Applications

Use `thiserror` to define typed, matchable error enums in library code. Use `anyhow` for application code where ergonomic error propagation with context matters.

**Incorrect (library exposes anyhow::Result in public API — callers cannot match on specific errors):**

```rust
// Library crate -- BAD: exposes anyhow in public API
pub fn parse_config(path: &str) -> anyhow::Result<Config> {
    let contents = std::fs::read_to_string(path)?;
    let config: Config = serde_json::from_str(&contents)?;
    Ok(config)
}
```

**Correct (library uses #[derive(Error)] enum with #[from]; application uses anyhow::Result with .context()):**

```rust
// --- library crate ---
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("could not read config file at {path}")]
    ReadFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid config format")]
    Parse(#[from] serde_json::Error),
}

pub fn parse_config(path: &str) -> Result<Config, ConfigError> {
    let contents = std::fs::read_to_string(path)
        .map_err(|e| ConfigError::ReadFile { path: path.to_string(), source: e })?;
    let config: Config = serde_json::from_str(&contents)?;
    Ok(config)
}

// --- application (main.rs) ---
use anyhow::{Context, Result};

fn run() -> Result<()> {
    let cfg = parse_config("app.toml")
        .context("failed to load application config")?;
    // callers can still match: if let Err(ConfigError::Parse(_)) = parse_config(...)
    Ok(())
}
```

**Note:** Many projects use both. Internal modules use `thiserror`; `main.rs` uses `anyhow`. Never expose `anyhow::Error` in a public library API — it erases type information that callers need to handle errors programmatically.

**thiserror 2.x update (current version):** Use `thiserror = "2"` in Cargo.toml. Key changes from 1.x: (1) `no_std` support via `default-features = false` on Rust 1.81+ (where `core::error::Error` is stable); (2) raw identifiers in format strings like `{r#type}` must now be written as `{type}`; (3) code using `derive(Error)` must have a direct dependency on `thiserror`.
