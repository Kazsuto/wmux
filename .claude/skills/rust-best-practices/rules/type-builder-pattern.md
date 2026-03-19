---
title: Builder Pattern for Complex Construction
impact: MEDIUM
impactDescription: clear, self-documenting API for structs with many optional parameters
tags: type, builder, construction, api-design, ergonomics
---

## Builder Pattern for Complex Construction

When a struct has many optional parameters, use the builder pattern instead of constructors with many positional arguments.

**Incorrect (positional arguments — unclear meaning at call site, hard to extend):**

```rust
struct Server {
    host: String,
    port: u16,
    timeout_secs: u64,
    max_connections: usize,
    tls_enabled: bool,
}

impl Server {
    fn new(host: String, port: u16, timeout_secs: u64, max_connections: usize, tls_enabled: bool) -> Self {
        Server { host, port, timeout_secs, max_connections, tls_enabled }
    }
}

fn main() {
    // What does each argument mean? Easy to swap timeout and max_connections.
    let _server = Server::new("0.0.0.0".to_string(), 8080, 30, 100, true);
}
```

**Correct (builder pattern — self-documenting, optional fields have defaults):**

```rust
#[derive(Debug)]
struct Server {
    host: String,
    port: u16,
    timeout_secs: u64,
    max_connections: usize,
    tls_enabled: bool,
}

#[derive(Debug)]
struct ServerBuilder {
    host: String,
    port: u16,
    timeout_secs: u64,
    max_connections: usize,
    tls_enabled: bool,
}

#[derive(Debug)]
struct BuildError(String);

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BuildError: {}", self.0)
    }
}

impl ServerBuilder {
    fn new() -> Self {
        ServerBuilder {
            host: "127.0.0.1".to_string(),
            port: 8080,
            timeout_secs: 30,
            max_connections: 100,
            tls_enabled: false,
        }
    }

    // Accept impl Into<String> so callers can pass &str or String
    fn host(mut self, host: impl Into<String>) -> Self {
        self.host = host.into();
        self
    }

    fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    fn timeout_secs(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    fn max_connections(mut self, max: usize) -> Self {
        self.max_connections = max;
        self
    }

    fn tls_enabled(mut self, enabled: bool) -> Self {
        self.tls_enabled = enabled;
        self
    }

    fn build(self) -> Result<Server, BuildError> {
        if self.port == 0 {
            return Err(BuildError("port cannot be 0".to_string()));
        }
        Ok(Server {
            host: self.host,
            port: self.port,
            timeout_secs: self.timeout_secs,
            max_connections: self.max_connections,
            tls_enabled: self.tls_enabled,
        })
    }
}

impl Server {
    fn builder() -> ServerBuilder {
        ServerBuilder::new()
    }
}

fn main() {
    let server = Server::builder()
        .host("0.0.0.0")
        .port(8080)
        .timeout_secs(60)
        .max_connections(500)
        .tls_enabled(true)
        .build()
        .expect("valid server config");

    println!("{:?}", server);
}
```

**Note:** Accept `impl Into<String>` in setters so callers can pass `&str` or `String` without explicit conversion. For structs with many fields, consider the `derive_builder` or `bon` crates to auto-generate the builder from the struct definition.
