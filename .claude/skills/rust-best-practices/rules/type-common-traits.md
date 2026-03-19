---
title: Eagerly Implement Common Standard Traits
impact: HIGH
impactDescription: enables Debug printing, comparison, hashing, and default construction
tags: type, traits, derive, debug, display, api-guidelines
---

## Eagerly Implement Common Standard Traits

Per Rust API Guidelines (C-COMMON-TRAITS): implement `Debug`, `Clone`, `Default`, `PartialEq`, `Eq`, `Hash` on all public types where semantically appropriate.

**Incorrect (no derives — unusable in tests, `HashMap` keys, or debug output):**

```rust
pub struct Config {
    pub host: String,
    pub port: u16,
    pub max_connections: usize,
}

fn main() {
    let config = Config {
        host: "localhost".to_string(),
        port: 8080,
        max_connections: 100,
    };
    // println!("{:?}", config); // compile error: Config doesn't implement Debug
    // let config2 = config.clone(); // compile error: Config doesn't implement Clone
    // assert_eq!(config, config); // compile error: Config doesn't implement PartialEq
}
```

**Correct (derive common traits eagerly):**

```rust
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub max_connections: usize,
}

// Implement Display separately for user-facing output
impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{} (max_conn={})", self.host, self.port, self.max_connections)
    }
}

fn main() {
    let config = Config {
        host: "localhost".to_string(),
        port: 8080,
        max_connections: 100,
    };

    println!("{:?}", config);       // Debug
    println!("{}", config);         // Display
    let config2 = config.clone();   // Clone
    assert_eq!(config, config2);    // PartialEq

    // Usable as HashMap key because of Eq + Hash
    let mut map = std::collections::HashMap::new();
    map.insert(config2, "production");

    // Default::default() gives Config { host: "", port: 0, max_connections: 0 }
    let default_config = Config::default();
    println!("{:?}", default_config);
}
```

**Note:** `Debug` should be derived on virtually every type. `Display` is for user-facing output and error messages. For types containing `f32`/`f64`, you cannot derive `Eq` or `Hash` directly — consider wrapping floats with the `ordered-float` crate's `OrderedFloat<f64>` instead. Also derive `Copy` on small, stack-allocated types where implicit copying is desired. `Send` and `Sync` are auto-traits implemented automatically when all fields satisfy the requirements — verify your types are `Send + Sync` when used in concurrent contexts. For types marked `#[non_exhaustive]`, external users must use `Default::default()` or a constructor since they cannot construct the struct directly.
