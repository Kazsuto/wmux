---
title: Typestate Pattern for Compile-Time State Enforcement
impact: HIGH
impactDescription: makes invalid states unrepresentable at compile time
tags: type, typestate, state-machine, phantom-data, safety
---

## Typestate Pattern for Compile-Time State Enforcement

Encode state transitions in the type system using generic type parameters and `PhantomData`. Methods only valid in certain states become compile-time enforced.

**Incorrect (runtime flag allows calling `send` on a disconnected connection):**

```rust
struct Connection {
    addr: String,
    connected: bool,
}

impl Connection {
    fn new(addr: &str) -> Self {
        Connection { addr: addr.to_string(), connected: false }
    }

    fn connect(&mut self) {
        self.connected = true;
    }

    fn send(&self, msg: &str) {
        if !self.connected {
            panic!("not connected"); // runtime error only
        }
        println!("Sending: {}", msg);
    }
}

fn main() {
    let conn = Connection::new("127.0.0.1:8080");
    conn.send("hello"); // panics at runtime — not caught at compile time
}
```

**Correct (`send` only exists on `Connection<Connected>`, enforced at compile time):**

```rust
use std::marker::PhantomData;

struct Disconnected;
struct Connected;

struct Connection<State> {
    addr: String,
    _state: PhantomData<State>,
}

impl Connection<Disconnected> {
    fn new(addr: &str) -> Self {
        Connection {
            addr: addr.to_string(),
            _state: PhantomData,
        }
    }

    fn connect(self) -> Connection<Connected> {
        println!("Connecting to {}", self.addr);
        Connection {
            addr: self.addr,
            _state: PhantomData,
        }
    }
}

impl Connection<Connected> {
    fn send(&self, msg: &str) {
        println!("Sending '{}' to {}", msg, self.addr);
    }

    fn disconnect(self) -> Connection<Disconnected> {
        println!("Disconnecting from {}", self.addr);
        Connection {
            addr: self.addr,
            _state: PhantomData,
        }
    }
}

fn main() {
    let conn = Connection::new("127.0.0.1:8080");
    // conn.send("hello"); // compile error: method not found on Connection<Disconnected>
    let conn = conn.connect();
    conn.send("hello"); // fine
}
```

**Note:** `connect()` consumes `self` (ownership transfer), so the old `Connection<Disconnected>` ceases to exist — you cannot hold two states simultaneously. This pattern is used in production crates such as `hyper` and `lettre`.
