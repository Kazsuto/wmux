---
title: Add Context to Errors with .context()
impact: CRITICAL
impactDescription: transforms cryptic OS errors into actionable debugging information
tags: error, context, anyhow, debugging, propagation
---

## Add Context to Errors with .context()

Bare `?` propagation loses information about what operation was attempted. Always attach context describing the high-level operation that failed.

**Incorrect (bare ? — error says "No such file or directory" with no info about which file or why it was read):**

```rust
use anyhow::Result;

fn load_user_config(name: &str) -> Result<Config> {
    let path = format!("/etc/myapp/{name}.toml");
    // If this fails: "No such file or directory (os error 2)"
    // The caller has no idea which path failed or what it was used for.
    let contents = std::fs::read_to_string(&path)?;
    let config: Config = toml::from_str(&contents)?;
    Ok(config)
}
```

**Correct (.with_context() attaches the operation and relevant values to the error chain):**

```rust
use anyhow::{Context, Result};

fn load_user_config(name: &str) -> Result<Config> {
    let path = format!("/etc/myapp/{name}.toml");

    // Lazy format: only allocates the string if an error actually occurs.
    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read config for '{name}' at {path}"))?;

    // Eager static string: fine when no formatting is needed.
    let config: Config = toml::from_str(&contents)
        .context("config file is not valid TOML")?;

    Ok(config)
}
// Error chain: "failed to read config for 'alice' at /etc/myapp/alice.toml"
//   caused by: "No such file or directory (os error 2)"
```

**Note:** Use `.with_context(|| ...)` (lazy closure) when the message requires formatting — this avoids allocating the String on the happy path. Use `.context("...")` (eager) only for static string slices. Both are provided by the `anyhow::Context` trait, which also works with any `Result<T, E: Error + Send + Sync + 'static>`, not just `anyhow::Result`. The trait is also implemented for `Option<T>`, converting `None` into an `anyhow::Error` with the given context message.
